use excalidraw_render::{ColorMode, OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn artifact(rgba: [u8; 4], mode: ColorMode, format: OutputFormat) -> Value {
    let [r, g, b, a] = rgba;
    artifact_css(&format!("#{r:02x}{g:02x}{b:02x}{a:02x}"), mode, format)
}

fn artifact_css(css: &str, mode: ColorMode, format: OutputFormat) -> Value {
    let scene = SceneDocument::parse(
        &serde_json::to_vec(&json!({
            "type":"excalidraw","version":2,"elements":[{
                "id":"swatch","type":"rectangle","x":0,"y":0,"width":20,"height":20,
                "strokeColor":"transparent","backgroundColor":css,
                "fillStyle":"solid","roughness":0,"seed":1,"updated":1
            }],"appState":{},"files":{}
        }))
        .unwrap(),
    )
    .unwrap();
    let request = RenderRequest::builder(format)
        .color_mode(mode)
        .include_background(false)
        .build()
        .unwrap();
    let output = Renderer::new().render(&scene, &request).unwrap();
    assert!(output.report().is_empty());
    let bytes = output.artifact().bytes();
    let pixels = if format == OutputFormat::Png {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).unwrap();
        json!({"sha256":format!("{:x}",Sha256::digest(&buffer[..info.buffer_size()])),"center":&buffer[((info.height/2*info.width+info.width/2)*4) as usize..][..4]})
    } else {
        Value::Null
    };
    json!({"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),"pixels":pixels})
}

pub(super) fn report(css: &Value, boundaries: &Value, sweep: &Value) -> Value {
    // Canonical-boundary projection: feed each result through an exact #RRGGBBAA
    // spelling to the unchanged public renderer. This measures propagation,
    // not an end-to-end replacement of every renderer color consumer.
    let mut inputs = vec!["color-mix(in oklab, red, blue)".to_string()];
    for row in css.as_array().unwrap() {
        if row["rgba"][0] != row["rgba"][1] || row["rgba"][0] != row["rgba"][2] {
            inputs.push(row["css"].as_str().unwrap().to_string());
        }
    }
    for rows in [boundaries, sweep] {
        for row in rows["first_difference"].as_array().unwrap() {
            if let Some(s) = row["css"].as_str() {
                inputs.push(s.to_string());
            }
        }
    }
    inputs.sort();
    inputs.dedup();
    json!(inputs.iter().map(|s| {
        let rgba = super::parsed(s).map(Option::unwrap);
        let mut observations=Vec::new();
        for mode in [ColorMode::Light,ColorMode::Dark] {
            for format in [OutputFormat::Svg,OutputFormat::Png,OutputFormat::Pdf] {
                let outputs = rgba.map(|c|artifact(c,mode,format));
                assert_eq!(outputs[0], artifact_css(s, mode, format), "canonical-boundary projection must equal actual CSS rendering: {s}");
                observations.push(json!({"mode":format!("{mode:?}"),"format":format!("{format:?}"),"outputs":outputs}));
            }
        }
        json!({"css":s,"rgba":rgba,"observations":observations})
    }).collect::<Vec<_>>())
}
