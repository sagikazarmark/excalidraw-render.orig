// Authored ICC 4.4/PNG bytes. No CMS supplies profiles or expected outcomes.
import { deflateSync, inflateSync } from "node:zlib";
import { fixtures, sceneFor } from "./png-color-fixtures.mjs";
export { sceneFor };
const u32 = n => { const b=Buffer.alloc(4);b.writeUInt32BE(n);return b; };
const u16 = n => { const b=Buffer.alloc(2);b.writeUInt16BE(n);return b; };
const fixed = n => {const b=Buffer.alloc(4);b.writeInt32BE(Math.round(n*65536));return b;};
function chunk(kind,data) {
  const body=Buffer.concat([Buffer.from(kind),data]);let crc=0xffffffff;
  for(const b of body){crc^=b;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}
  return Buffer.concat([u32(data.length),body,u32((crc^0xffffffff)>>>0)]);
}
const xyz = v => Buffer.concat([Buffer.from("XYZ \0\0\0\0"),...v.map(fixed)]);
const curve = (version,gamma) => version===2 ? Buffer.concat([Buffer.from("curv\0\0\0\0"),u32(1),u16(Math.round(gamma*256)),u16(0)])
  : Buffer.concat([Buffer.from("para\0\0\0\0"),u32(0),fixed(gamma)]);
function profile(version,model,tags,klass="mntr",intent=0) {
  const head=Buffer.alloc(128);head[8]=version;head[9]=0x30;head.write(klass,12);head.write(model,16);head.write("XYZ ",20);head.write("acsp",36);head.writeUInt32BE(intent,64);
  Buffer.concat([fixed(.9642),fixed(1),fixed(.8249)]).copy(head,68);
  let offset=132+tags.length*12;const records=[],bodies=[];
  for(const [name,data] of tags){records.push(Buffer.concat([Buffer.from(name),u32(offset),u32(data.length)]));const padded=Buffer.alloc(Math.ceil(data.length/4)*4);data.copy(padded);bodies.push(padded);offset+=padded.length;}
  const bytes=Buffer.concat([head,u32(tags.length),...records,...bodies]);bytes.writeUInt32BE(bytes.length);return bytes;
}
function lut(kind,grid=2,variant=0) {
  const values=[];
  for(let r=0;r<grid;r++)for(let g=0;g<grid;g++)for(let b=0;b<grid;b++){
    const rgb=[r,g,b].map(v=>v/(grid-1));if(variant===1)rgb[0]=1-rgb[0];if(variant===2)rgb.reverse();
    for(const coefficients of [[.436035,.385117,.143051],[.222488,.716904,.060608],[.013916,.097061,.713913]])values.push(coefficients.reduce((s,v,i)=>s+v*rgb[i],0));
  }
  if(kind==="mAB "){
    // A identity curves -> CLUT -> B identity curves, XYZ u1Fixed15 PCS.
    const h=Buffer.alloc(32);h.write(kind);h[8]=3;h[9]=3;h.writeUInt32BE(32,12);h.writeUInt32BE(80,24);h.writeUInt32BE(32,28);
    const clut=Buffer.alloc(20);clut.fill(grid,0,3);clut[16]=2;
    return Buffer.concat([h,...Array.from({length:3},()=>curve(4,1)),clut,...values.map(v=>u16(Math.round(v*32768)))]);
  }
  const h=Buffer.alloc(kind==="mft1"?48:52);h.write(kind);h[8]=3;h[9]=3;h[10]=grid;
  for(let i=0;i<9;i++)fixed(i%4===0?1:0).copy(h,12+i*4);
  if(kind==="mft1")return Buffer.concat([h,...Array.from({length:3},()=>Buffer.from(Array.from({length:256},(_,i)=>i))),Buffer.from(values.map(v=>Math.round(v*255))),...Array.from({length:3},()=>Buffer.from(Array.from({length:256},(_,i)=>i)))]);
  h.writeUInt16BE(2,48);h.writeUInt16BE(2,50);
  return Buffer.concat([h,...Array.from({length:3},()=>Buffer.from([0,0,255,255])),...values.map(v=>u16(Math.round(v*32768))),...Array.from({length:3},()=>Buffer.from([0,0,255,255]))]);
}
function replace(row,tags,id,group,outcome="accepted") {
  const rewrite = encoded => {const source=Buffer.from(encoded,"base64");return Buffer.concat([source.subarray(0,33),...tags.map(t=>chunk(t.kind,Buffer.from(t.data))),source.subarray(33)]).toString("base64");};
  return {...row,id:`${id}-c${row.layout.color_type}-d${row.layout.depth}`,signal:id,group,expected_outcome:outcome,tags,png:rewrite(row.png),opaque_png:rewrite(row.opaque_png)};
}
const tag = bytes => ({kind:"iCCP",data:[...Buffer.concat([Buffer.from("authored\0\0"),deflateSync(bytes)])],after:false});
export function stage4Fixtures(input) {
  const base=fixtures({...input,cicp_primaries:[]}).filter(r=>r.signal==="untagged");const rows=[];
  const add=(id,group,tags,outcome="accepted",layouts=base)=>{for(const row of layouts)rows.push(replace(row,tags,id,group,outcome));};
  const white=["wtpt",xyz([.9642,1,.8249])];
  for(const version of [2,4])for(const gamma of [1,2.2]){
    const p=profile(version,"GRAY",[white,["kTRC",curve(version,gamma)]]);
    add(`gray-v${version}-g${gamma}`,"icc-gray",[tag(p)],"accepted",base.filter(r=>r.layout.color_type===4));
    add(`gray-on-rgb-v${version}-g${gamma}`,"icc-model",[tag(p)],"unsupported-format",base.filter(r=>r.layout.color_type===6));
  }
  for(const kind of ["mft1","mft2","mAB "])for(const grid of [2,3])for(const intent of [0,1,2,3]){
    const p=profile(kind==="mAB "?4:2,"RGB ",[white,["A2B0",lut(kind,grid)],["A2B1",lut(kind,grid,1)],["A2B2",lut(kind,grid,2)]],"mntr",intent);
    add(`lut-${kind.trim()}-${grid}-intent${intent}`,"icc-lut",[tag(p)],"accepted",base.filter(r=>r.layout.color_type===6));
  }
  const linear=inflateSync(Buffer.from(fixtures({...input,cicp_primaries:[]}).find(r=>r.signal==="icc-v2-gamma1").tags[0].data).subarray(10));
  for(const model of ["CMYK","Lab ","XYZ ","2CLR","5CLR"]){const p=Buffer.from(linear);p.write(model,16);add(`model-${model.trim()}`,"icc-model",[tag(p)],"unsupported-format");}
  for(const klass of ["link","abst","nmcl"]){const p=Buffer.from(linear);p.write(klass,12);add(`class-${klass}`,"icc-model",[tag(p)],"unsupported-format");}
  const badLut=lut("mAB ");badLut.write("mBA ");
  add("lut-wrong-direction","icc-lut",[tag(profile(4,"RGB ",[white,["A2B0",badLut]]))],"unsupported-format");
  const mpet=Buffer.concat([Buffer.from("mpet\0\0\0\0"),u16(3),u16(3),u32(0)]);
  add("lut-mpet-only","icc-lut",[tag(profile(4,"RGB ",[white,["D2B0",mpet]]))],"unsupported-format");
  const signals=[
    ["name-missing-nul",Buffer.from("authored"),"accepted"],
    ["empty-name",Buffer.concat([Buffer.from([0,0]),deflateSync(linear)]),"accepted"],
    ["name-80",Buffer.concat([Buffer.alloc(80,65),Buffer.from([0,0]),deflateSync(linear)]),"accepted"],
    ["method",Buffer.concat([Buffer.from("authored\0\x01"),deflateSync(linear)]),"accepted"],
    ["stream",Buffer.from("authored\0\0broken"),"accepted"],
    ["profile-short",Buffer.concat([Buffer.from("authored\0\0"),deflateSync(linear.subarray(0,100))]),"unsupported-format"],
    ["profile-empty",Buffer.concat([Buffer.from("authored\0\0"),deflateSync(Buffer.alloc(0))]),"unsupported-format"],
  ];
  for(const [id,data,outcome] of signals)add(id,"icc-malformed",[{kind:"iCCP",data:[...data],after:false}],outcome);
  add("duplicate-first-linear","icc-malformed",[tag(linear),tag(Buffer.alloc(0))]);
  add("duplicate-first-invalid","icc-malformed",[tag(Buffer.alloc(0)),tag(linear)],"unsupported-format");
  const rgb=base.filter(r=>r.layout.color_type===6);
  for(const selected of [1,2]) {
    add(`lut-only-a2b${selected}`,"icc-lut-extra",[tag(profile(2,"RGB ",[white,[`A2B${selected}`,lut("mft2",3,selected)]]))],"accepted",rgb);
  }
  for(const kind of ["mft1","mft2","mAB "]) {
    const table=lut(kind,3);
    const offset=kind==="mft1"?48+768:kind==="mft2"?52+12:100;
    // Deliberately non-matrix RGB -> Lab CLUT: L from R, a from G, b from B.
    // Encoded Lab v2 lut16 uses 65280, whereas lut8/v4 use their full range.
    let i=offset;
    for(let r=0;r<3;r++)for(let g=0;g<3;g++)for(let b=0;b<3;b++)for(const v of [r/2,.35+.3*g/2,.35+.3*b/2]) {
      if(kind==="mft1")table[i++]=Math.round(v*255);else {table.writeUInt16BE(Math.round(v*(kind==="mft2"?65280:65535)),i);i+=2;}
    }
    const p=profile(kind==="mAB "?4:2,"RGB ",[white,["A2B0",table]]);p.write("Lab ",20);
    add(`lut-lab-${kind.trim()}`,"icc-lut-extra",[tag(p)],"accepted",rgb);
  }
  const bOnly=Buffer.alloc(32);bOnly.write("mAB ");bOnly[8]=bOnly[9]=3;bOnly.writeUInt32BE(32,12);
  add("lut-mab-b-curves","icc-lut-extra",[tag(profile(4,"RGB ",[white,["A2B0",Buffer.concat([bOnly,...Array.from({length:3},()=>curve(4,2.2))])]]))],"accepted",rgb);
  const stages=lut("mAB ",3),matrixOffset=stages.length;
  stages.writeUInt32BE(matrixOffset,16);stages.writeUInt32BE(matrixOffset+48,20);
  const matrix=Buffer.concat([...Array.from({length:9},(_,i)=>fixed(i%4===0?.9:0)),...Array.from({length:3},()=>fixed(.01))]);
  add("lut-mab-matrix-m-curves","icc-lut-extra",[tag(profile(4,"RGB ",[white,["A2B0",Buffer.concat([stages,matrix,...Array.from({length:3},()=>curve(4,2.2))])]]))],"unsupported-format",rgb);
  // Matrix/curve offsets must be four-byte aligned. Keep the original
  // deliberately strict-recovery vector and add the properly aligned pipeline.
  const aligned=Buffer.alloc(Math.ceil(stages.length/4)*4);stages.copy(aligned);aligned.writeUInt32BE(aligned.length,16);aligned.writeUInt32BE(aligned.length+48,20);
  add("lut-mab-matrix-m-curves-aligned","icc-lut-extra",[tag(profile(4,"RGB ",[white,["A2B0",Buffer.concat([aligned,matrix,...Array.from({length:3},()=>curve(4,2.2))])]]))],"accepted",rgb);
  const link=profile(4,"RGB ",[["A2B0",lut("mft2")]],"link");link.write("RGB ",20);
  add("device-link-rgb","icc-model-extra",[tag(link)],"unsupported-format");
  for(const [name,at,value] of [["signature",36,0],["table-count",128,0xffffffff],["tag-offset",136,0xffffffff],["tag-size",140,0xffffffff],["declared-size",0,0xffffffff]]){
    const p=Buffer.from(linear);p.writeUInt32BE(value,at);add(`icc-${name}`,"icc-malformed-extra",[tag(p)],"unsupported-format");
  }
  // A valid ICC multiProcessElementsType with one 3->3 float matrix element,
  // not merely an empty or unknown tag. D2B0 alone is an unsupported source
  // class here; alongside A2B0 the pinned browser selects the A2B pipeline.
  const f32=n=>{const b=Buffer.alloc(4);b.writeFloatBE(n);return b;};
  const matf=Buffer.concat([Buffer.from("matf\0\0\0\0"),u16(3),u16(3),
    ...[.436035,.385117,.143051,.222488,.716904,.060608,.013916,.097061,.713913,0,0,0].map(f32)]);
  const processes=Buffer.concat([Buffer.from("mpet\0\0\0\0"),u16(3),u16(3),u32(1),u32(24),u32(matf.length),matf]);
  add("lut-mpet-matrix","icc-lut",[tag(profile(4,"RGB ",[white,["D2B0",processes]]))],"unsupported-format");
  add("lut-a2b-over-mpet","icc-lut",[tag(profile(4,"RGB ",[white,["D2B0",processes],["A2B0",lut("mft2",3)]]))],"accepted",rgb);
  return rows;
}
