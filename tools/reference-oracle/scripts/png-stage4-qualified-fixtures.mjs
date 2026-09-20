// The pinned browser ignores late eXIf. Color-composition evidence uses early
// eXIf; exact late-placement equivalence is independently checked by Rust byte
// contracts. Keep the original late-observation collector and record intact.
import {compositionFixtures,sceneFor} from "./png-stage4-composition-fixtures.mjs";
export {sceneFor};
export function qualifiedFixtures(input){
 return compositionFixtures(input).map(row=>{
  const early=encoded=>{const b=Buffer.from(encoded,"base64"),chunks=[];for(let at=8;at<b.length;){const n=b.readUInt32BE(at)+12;chunks.push(b.subarray(at,at+n));at+=n;}
   const exif=chunks.findIndex(c=>c.toString("ascii",4,8)==="eXIf"),idat=chunks.findIndex(c=>c.toString("ascii",4,8)==="IDAT");
   if(exif>idat){const [metadata]=chunks.splice(exif,1);chunks.splice(idat,0,metadata);}
   return Buffer.concat([b.subarray(0,8),...chunks]).toString("base64");};
  return {...row,id:`qualified-${row.id}`,signal:`qualified-${row.signal}`,png:early(row.png),opaque_png:early(row.opaque_png)};
 });
}
