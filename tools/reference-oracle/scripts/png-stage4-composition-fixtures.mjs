// Independent encoded-layout/profile/transparency/subframe crossings.
import {deflateSync} from "node:zlib";
import {fixtures,sceneFor} from "./png-color-fixtures.mjs";
import {stage4Fixtures} from "./png-stage4-fixtures.mjs";
export {sceneFor};
const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;};
function chunk(kind,data){const body=Buffer.concat([Buffer.from(kind),data]);let crc=0xffffffff;for(const b of body){crc^=b;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return Buffer.concat([u32(data.length),body,u32((crc^0xffffffff)>>>0)]);}
function exif(o){return Buffer.from([0x49,0x49,42,0,8,0,0,0,1,0,0x12,1,3,0,1,0,0,0,o,0,0,0,0,0,0,0]);}
function control(seq,w,h,x,y){const b=Buffer.alloc(26);[seq,w,h,x,y].forEach((v,i)=>b.writeUInt32BE(v,i*4));b.writeUInt16BE(65535,20);b.writeUInt16BE(1,22);b[24]=2;b[25]=1;return b;}
const configurations=[
 ["gray-v2-g1",true],["gray-v4-g2.2",true],
 ...["lut-mft1-3-intent3","lut-mft2-2-intent0","lut-lab-mAB","lut-mab-b-curves","lut-mab-matrix-m-curves-aligned","lut-only-a2b1","lut-only-a2b2","lut-a2b-over-mpet","gama-70000","chrm-p3-100000","icc-v4-param2","icc-v4-srgb","cicp-p9-t16-r1","cicp-p10-t18-r1"].map(s=>[s,false]),
];
export function compositionFixtures(input){
 const sources=[...fixtures(input),...stage4Fixtures(input)];const result=[];
 for(const [signal,gray] of configurations){
  const source=sources.find(r=>r.signal===signal&&r.layout.depth===8&&r.layout.color_type===(gray?4:6));if(!source)throw Error(signal);
  const layouts=[];for(const depth of [8,16])for(const alpha of ["intrinsic","key","none"])layouts.push({depth,alpha,color:gray?(alpha==="intrinsic"?4:0):(alpha==="intrinsic"?6:2)});
  if(gray){for(const depth of [1,2,4])for(const alpha of ["key","none"])layouts.push({depth,alpha,color:0});}
  else for(const depth of [1,2,4,8])layouts.push({depth,alpha:"palette",color:3});
  for(const layout of layouts)for(const adam7 of [false,true])for(const container of ["static","included","subframe"])for(const orientation of [6,7]){
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
   const id=`cross-${signal}-c${layout.color}-d${layout.depth}-${layout.alpha}-${adam7}-${container}-o${orientation}`;
   result.push({id,group:"composition",signal:id,layout:{color_type:layout.color,depth:layout.depth},width:h,height:w,alpha:orientedAlpha,tags:source.tags,expected_outcome:"accepted",animation_frames:container==="static"?1:2,png:encode(false),opaque_png:encode(true)});
  }
 }
 return result;
}
