//! An element's own PNG alpha mask — a rounded or rotated image's clip, the
//! clip of a missing or undecodable file's placeholder, an arrow's bound-label
//! exclusion — costs work in proportion to the element rather than the canvas,
//! as the resource ledger charges it, and paints the bytes a full-canvas mask
//! per element painted.

use std::time::{Duration, Instant};

use excalidraw_render::{DiagnosticCode, OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::common::lower_hex;

/// A 6x6 RGBA PNG with varied colours and some translucent pixels.
const IMAGE_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAYAAAAGCAYAAADgzO9IAAAAUElEQVR4nBWMQQ0AQQwCkVM5lcOzUpCDq7vZx2RICEg6j+5bMDm4IM19Aztn40DJ0lLsebEh5O5bmCuzAJOD61eEBWy4woHmXZWiXGFDyMU/NwZLqTNG8CcAAAAASUVORK5CYII=";

/// The elements of masked cell `index` at (`x`, `y`), inside `frame` if any.
/// Cells cycle through every kind of element mask and one image that needs
/// none, so consecutive elements switch between them.
fn masked_cell(index: usize, x: f64, y: f64, frame: Option<&str>) -> Vec<Value> {
    let id = format!("cell-{index}");
    let mut element = json!({
        "id": id, "type": "image", "x": x, "y": y, "width": 37.3, "height": 23.7,
        "angle": 0, "opacity": 100, "status": "saved", "scale": [1, 1],
        "fileId": "image", "seed": index + 1, "updated": 1, "frameId": frame
    });
    let mut label = None;
    match index % 6 {
        0 => element["roundness"] = json!({"type": 3}),
        1 => element["angle"] = json!(0.3 + 0.07 * (index % 11) as f64),
        2 => {
            element["fileId"] = json!("missing");
            element["angle"] = json!(0.05 * (index % 13) as f64);
        }
        3 => {
            element["fileId"] = json!("undecodable");
            element["roundness"] = json!({"type": 3});
            element["angle"] = json!(0.5);
        }
        4 => {
            element = json!({
                "id": id, "type": "arrow", "x": x, "y": y, "width": 90, "height": 35,
                "points": [[0, 0], [45, 10], [90, 35]], "roundness": {"type": 2},
                "boundElements": [{"type": "text", "id": format!("{id}-label")}],
                "roughness": 1, "seed": index + 1, "endArrowhead": "arrow", "updated": 1,
                "frameId": frame
            });
            label = Some(json!({
                "id": format!("{id}-label"), "type": "text", "x": x + 30.0, "y": y + 5.0,
                "width": 30, "height": 25, "containerId": id, "fontSize": 20, "fontFamily": 5,
                "text": "L", "textAlign": "center", "verticalAlign": "middle",
                "lineHeight": 1.25, "updated": 1, "frameId": frame
            }));
        }
        _ => element["scale"] = json!([-1, 1]),
    }
    std::iter::once(element).chain(label).collect()
}

fn scene(elements: Vec<Value>) -> SceneDocument {
    let source = json!({
        "type": "excalidraw", "version": 2, "elements": elements,
        "appState": {"viewBackgroundColor": "#ffffff"},
        "files": {
            "image": {"id": "image", "mimeType": "image/png", "dataURL": IMAGE_DATA_URL, "created": 1},
            "undecodable": {
                "id": "undecodable", "mimeType": "image/png",
                "dataURL": "data:image/png;base64,iVBORw0KGgo=", "created": 1
            }
        }
    });
    SceneDocument::parse(source.to_string().as_bytes()).expect("masked scene is admitted")
}

fn render_png(scene: &SceneDocument) -> Vec<u8> {
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("default PNG request is valid");
    let outcome = Renderer::new()
        .render(scene, &request)
        .expect("masked scene renders at default limits");
    // The placeholders report their files; nothing is omitted or replaced.
    assert!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .all(|diagnostic| matches!(
                diagnostic.code(),
                DiagnosticCode::IMAGE_MISSING_FILE | DiagnosticCode::IMAGE_DECODE_FAILED
            )),
        "masked elements were not all drawn: {:?}",
        outcome.report().diagnostics()
    );
    outcome.artifact().bytes().to_vec()
}

#[test]
fn element_masks_paint_the_bytes_of_a_full_canvas_mask() {
    // Two frames clip some cells, one across its edge, others are unframed,
    // and a row straddles x = 2048: the 4x raster is wider than 8191 pixels,
    // where tiny-skia rasterizes in translated tiles.
    let mut elements = vec![
        json!({"id": "left", "type": "frame", "name": "Left", "x": 0, "y": 0,
               "width": 500, "height": 160, "updated": 1}),
        json!({"id": "seam", "type": "frame", "name": "Seam", "x": 1900, "y": 0,
               "width": 300, "height": 200, "updated": 1}),
    ];
    let mut index = 0;
    let mut cells = |elements: &mut Vec<Value>, x: f64, y: f64, frame: Option<&str>| {
        elements.extend(masked_cell(index, x, y, frame));
        index += 1;
    };
    for row in 0..3 {
        for column in 0..5 {
            let (x, y) = (10.0 + 110.0 * column as f64, 10.0 + 60.0 * row as f64);
            cells(
                &mut elements,
                x + 0.37 * row as f64,
                y + 0.21 * column as f64,
                Some("left"),
            );
        }
    }
    for row in 0..3 {
        for column in 0..4 {
            let (x, y) = (1930.0 + 75.0 * column as f64, 10.0 + 70.0 * row as f64);
            cells(&mut elements, x, y, Some("seam"));
        }
    }
    for column in 0..18 {
        cells(&mut elements, 520.0 + 90.0 * column as f64, 230.0, None);
        cells(
            &mut elements,
            2010.0 + 7.3 * column as f64,
            260.0 + 3.1 * column as f64,
            None,
        );
    }
    let png = render_png(&scene(elements));

    // The digest of the bytes the renderer produced while every masked
    // element built and intersected a full-canvas mask.
    assert_eq!(
        lower_hex(&Sha256::digest(&png)),
        "88a54503c5b714812d66940f260ef7f6746ada93fae1788417d3a7821d4ef65e"
    );
}

#[test]
fn many_element_masks_on_a_large_canvas_render_quickly() {
    // 8,000 masked elements over a 1,400 px square canvas, whose 4x raster is
    // 31 million pixels. A full-canvas mask each cost 3 to 6 ms, 25 s or more
    // in all; masks bounded by their elements render the scene in about 2 s.
    let mut elements = vec![
        json!({"id": "top-left", "type": "rectangle", "x": 0, "y": 0, "width": 8, "height": 8,
               "roughness": 0, "updated": 1}),
        json!({"id": "bottom-right", "type": "rectangle", "x": 1372, "y": 1372, "width": 8,
               "height": 8, "roughness": 0, "updated": 1}),
    ];
    for index in 0..8_000 {
        let (x, y) = ((index * 37 % 1_300) as f64, (index * 91 % 1_300) as f64);
        elements.extend(masked_cell(index, x, y, None));
    }
    let scene = scene(elements);

    let started = Instant::now();
    render_png(&scene);
    assert!(started.elapsed() < Duration::from_secs(10));
}
