use excalidraw_render::{OutputFormat,RenderRequest,Renderer,SceneDocument};
use excalidraw_render_compare::{compare,CanonicalRgba8,ComparisonMode,VisualClass,SsimOutcome};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{fs,io::Cursor,path::Path};
fn sha(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
fn main(){
 let root=Path::new("/home/laborant/excalidraw-render.orig/tests/conformance/cases/focused");
 let out=Path::new("/tmp/opencode/six-phase-probe");let mut rows=vec![];
 for case in ["embeddable-links","emoji-fallback","bidi-text","bidi-alignment","text-line-endings","text-alignment-vertical"]{
  let dir=root.join(case);let manifest:Value=serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
  let scene_bytes=fs::read(dir.join(manifest["scene"]["path"].as_str().unwrap())).unwrap();
  assert_eq!(sha(&scene_bytes),manifest["scene"]["sha256"].as_str().unwrap());
  let scene=SceneDocument::parse(&scene_bytes).unwrap();let raw=&manifest["variants"][0]["formats"]["png"]["raw_raster"];
  let reference_bytes=fs::read(dir.join(raw["path"].as_str().unwrap())).unwrap();assert_eq!(sha(&reference_bytes),raw["sha256"].as_str().unwrap());
  let w=raw["width"].as_u64().unwrap()as u32;let h=raw["height"].as_u64().unwrap()as u32;
  let reference=CanonicalRgba8::new(w,h,reference_bytes).unwrap();let mut pair=vec![];
  let expected:Value=serde_json::from_slice(&fs::read(dir.join("expected-report-png.json")).unwrap()).unwrap();
  let mut svg_sha=None;
  for mode in ["current","legacy"]{
   std::env::set_var("SIX_PHASE_MODE",mode);
   let request=RenderRequest::builder(OutputFormat::Png).build().unwrap();
   let result=Renderer::new().render(&scene,&request).unwrap();assert_eq!(serde_json::to_value(result.report()).unwrap(),expected);
   let mut decoder=png::Decoder::new(Cursor::new(result.artifact().bytes()));decoder.set_transformations(png::Transformations::EXPAND);
   let mut reader=decoder.read_info().unwrap();let mut bytes=vec![0;reader.output_buffer_size()];let info=reader.next_frame(&mut bytes).unwrap();assert_eq!((info.width,info.height),(w,h));bytes.truncate(info.buffer_size());
   let rgba=match info.color_type{png::ColorType::Rgba=>bytes,png::ColorType::Rgb=>bytes.chunks_exact(3).flat_map(|p|[p[0],p[1],p[2],255]).collect(),_=>panic!("color")};
   fs::write(out.join(format!("{case}-{mode}.rgba")),&rgba).unwrap();
   let report=compare(&reference,&CanonicalRgba8::new(w,h,rgba.clone()).unwrap(),VisualClass::GeometryImage,&[],ComparisonMode::Canonical).unwrap();
   let SsimOutcome::Score(ssim)=report.whole.ssim else{panic!()};
   let svg=Renderer::new().render(&scene,&RenderRequest::builder(OutputFormat::Svg).build().unwrap()).unwrap();let hash=sha(svg.artifact().bytes());
   if let Some(ref prior)=svg_sha{assert_eq!(prior,&hash)}else{svg_sha=Some(hash.clone())};
   println!("{case} {mode}: changed={} ssim={ssim} bits={:016x} canonical={} ranges=OK svg={hash}",report.whole.changed_pixels,ssim.to_bits(),report.passed());
   pair.push(rgba);rows.push(json!({"case":case,"mode":mode,"changed":report.whole.changed_pixels,"ssim":ssim,"ssim_bits":format!("{:016x}",ssim.to_bits()),"canonical":report.passed(),"svg_sha256":hash}));
  }
  let mut changes=vec![];for(i,(a,b))in pair[0].chunks_exact(4).zip(pair[1].chunks_exact(4)).enumerate(){if a!=b{changes.push((i%(w as usize),i/(w as usize)))}}
  println!("DELTA {case}: {} raw pixels bbox {:?}",changes.len(),[changes.iter().map(|p|p.0).min(),changes.iter().map(|p|p.1).min(),changes.iter().map(|p|p.0).max(),changes.iter().map(|p|p.1).max()]);
 }
 fs::write(out.join("measurements.json"),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}
