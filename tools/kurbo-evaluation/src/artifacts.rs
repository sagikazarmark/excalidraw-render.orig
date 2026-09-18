use excalidraw_render::{
    FrameOptions, OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
#[path = "../../../crates/excalidraw-render/tests/common/pdf_inspect.rs"]
mod pdf_inspect;

fn scene(elements: Value) -> SceneDocument {
    SceneDocument::parse(
        &serde_json::to_vec(&json!({
            "type":"excalidraw", "version":2, "elements":elements, "appState":{}, "files":{}
        }))
        .unwrap(),
    )
    .unwrap()
}

fn request(format: OutputFormat, limits: ResourceLimits) -> RenderRequest {
    RenderRequest::builder(format)
        .frame_options(FrameOptions::new(true, true, false, false))
        .resource_limits(limits)
        .build()
        .unwrap()
}

fn artifact(scene: &SceneDocument, format: OutputFormat) -> Value {
    let outcome = Renderer::new()
        .render(scene, &request(format, ResourceLimits::default()))
        .unwrap();
    assert!(
        outcome.report().is_empty(),
        "probe must render without recovery"
    );
    let bytes = outcome.artifact().bytes();
    let detail = match format {
        OutputFormat::Pdf => {
            let text = pdf_inspect::pdf_text(bytes);
            json!({"matrices":text.lines().filter(|line| line.ends_with(" cm")).collect::<Vec<_>>()})
        }
        OutputFormat::Png => {
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut pixels).unwrap();
            json!({"width":info.width,"height":info.height,"pixels_sha256":format!("{:x}",Sha256::digest(&pixels[..info.buffer_size()]))})
        }
        _ => Value::Null,
    };
    json!({"format":format!("{format:?}"),"length":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),"report":outcome.report(),"detail":detail})
}

fn limits(value: u64) -> ResourceLimits {
    ResourceLimits::builder()
        .max_element_work_units(value)
        .build()
        .unwrap()
}

fn threshold(scene: &SceneDocument) -> Value {
    // Minimal element-work budget giving a complete PNG (including recovery
    // reports, not just successful encoding). Then retain both boundary outcomes.
    let clean = |value| {
        Renderer::new()
            .render(scene, &request(OutputFormat::Png, limits(value)))
            .is_ok_and(|o| o.report().is_empty())
    };
    let (mut low, mut high) = (1, 2_000_000);
    assert!(clean(high));
    while low < high {
        let mid = low + (high - low) / 2;
        if clean(mid) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    assert!(low > 1 && !clean(low - 1));
    let at = Renderer::new()
        .render(scene, &request(OutputFormat::Png, limits(low)))
        .unwrap();
    let below = Renderer::new().render(scene, &request(OutputFormat::Png, limits(low - 1)));
    let below = match below {
        Ok(o) => {
            json!({"report":o.report(),"sha256":format!("{:x}",Sha256::digest(o.artifact().bytes()))})
        }
        Err(e) => json!({"error":format!("{e:?}")}),
    };
    json!({"minimum_clean_element_work":low,"at_report":at.report(),"below":below})
}

fn output_limit(scene: &SceneDocument, cap: u64) -> Value {
    let request = request(
        OutputFormat::Pdf,
        ResourceLimits::builder()
            .max_output_bytes(cap)
            .build()
            .unwrap(),
    );
    match Renderer::new().render(scene, &request) {
        Ok(outcome) => {
            json!({"cap":cap,"accepted":true,"report":outcome.report(),"bytes":outcome.artifact().bytes().len()})
        }
        Err(error) => json!({"cap":cap,"accepted":false,"error":format!("{error:?}")}),
    }
}

pub(super) fn report() -> Value {
    let mut scenes = Vec::new();
    for seed in 1..=32 {
        let elements = json!([{
            "id":"curve","type":if seed % 2 == 0 {"arrow"} else {"line"},
            "x":0,"y":0,"width":120,"height":100,
            "points":[[0,0],[120,40],[-20,100],[90,20]],
            "roundness":{"type":2},"angle":seed as f64 / 11.0,
            "roughness":(seed % 3) as f64,"seed":seed,"updated":1,
            "endArrowhead":if seed % 2 == 0 {json!("arrow")} else {Value::Null}
        }]);
        scenes.push((format!("rotated-linear-{seed}"), elements, false));
    }
    for (name, elements) in [
        (
            "frame-text",
            json!([
                {"id":"frame","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
                {"id":"text","type":"text","x":10,"y":20,"width":30,"height":25,"frameId":"frame","fontSize":20,"fontFamily":5,"text":"ab","lineHeight":1.25,"updated":1}
            ]),
        ),
        (
            "freedraw-solid",
            json!([
                {"id":"draw","type":"freedraw","x":0,"y":0,"width":100,"height":80,"points":[[0,0],[100,10],[80,80],[0,0]],"pressures":[0.5,0.7,0.4,0.5],"simulatePressure":false,"backgroundColor":"#ff0000","fillStyle":"solid","angle":0.7,"seed":77,"updated":1}
            ]),
        ),
        (
            "freedraw-sketch",
            json!([
                {"id":"draw","type":"freedraw","x":0,"y":0,"width":100,"height":80,"points":[[0,0],[100,10],[80,80],[0,0]],"pressures":[0.5,0.7,0.4,0.5],"simulatePressure":false,"backgroundColor":"#ff0000","fillStyle":"hachure","angle":0.7,"seed":77,"updated":1}
            ]),
        ),
        (
            "rounded-bound-label",
            json!([
                {"id":"arrow","type":"arrow","x":0,"y":0,"width":100,"height":120,"points":[[0,0],[100,60],[0,120]],"roundness":{"type":2},"angle":0.7,"boundElements":[{"type":"text","id":"label"}],"seed":77,"updated":1},
                {"id":"label","type":"text","x":30,"y":50,"width":40,"height":20,"containerId":"arrow","fontSize":16,"fontFamily":5,"text":"Label","textAlign":"center","verticalAlign":"middle","lineHeight":1.25,"updated":1}
            ]),
        ),
    ] {
        scenes.push((name.to_owned(), elements, true));
    }
    scenes.into_iter().map(|(name, elements, resource)| {
        let scene = scene(elements.clone());
        let artifacts: Vec<_> = [OutputFormat::Svg,OutputFormat::Png,OutputFormat::Pdf].into_iter().map(|f| artifact(&scene,f)).collect();
        let output_limit = match name.as_str() {
            "rotated-linear-5" => output_limit(&scene, 1997),
            "rotated-linear-17" => output_limit(&scene, 1999),
            _ => Value::Null,
        };
        json!({"name":name,"elements":elements,"outputs":artifacts,"resource":if resource {threshold(&scene)} else {Value::Null},"output_limit":output_limit})
    }).collect()
}
