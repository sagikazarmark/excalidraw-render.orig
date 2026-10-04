//! PNG frame clipping charges one shared alpha mask per frame rather than a
//! full-canvas mask for every clipped child (#112).

use excalidraw_render::{
    ErrorCode, FrameOptions, OutputFormat, RenderError, RenderErrorCategory, RenderOutcome,
    RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use serde_json::json;

/// A 100x60 frame with four children. With names and outlines disabled the
/// canvas is exactly 120x80 = 9,600 pixels and the frame clip covers 100x60 =
/// 6,000 device pixels.
const FRAMED_RECTANGLES: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"frame","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
        {"id":"a","type":"rectangle","x":10,"y":10,"width":20,"height":20,"frameId":"frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
        {"id":"b","type":"rectangle","x":40,"y":10,"width":20,"height":20,"frameId":"frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
        {"id":"c","type":"rectangle","x":70,"y":10,"width":20,"height":20,"frameId":"frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
        {"id":"d","type":"rectangle","x":10,"y":35,"width":20,"height":20,"frameId":"frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

const CANVAS_PIXELS: u64 = 120 * 80;
const FRAME_CLIP_PIXELS: u64 = 100 * 60;

/// The 100x60 frame with one text child; the canvas stays 120x80.
const FRAMED_TEXT: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"frame","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
        {"id":"caption","type":"text","x":10,"y":20,"width":30,"height":25,"frameId":"frame","strokeColor":"#000000","fontSize":20,"fontFamily":5,"text":"ab","textAlign":"left","verticalAlign":"top","lineHeight":1.25,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

/// An arrow with a reciprocal bound label and nothing else; the label mask
/// excludes only the label's box from the arrow.
const LABELLED_ARROW: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"arrow","type":"arrow","x":0,"y":20,"width":100,"height":0,"points":[[0,0],[100,0]],"boundElements":[{"type":"text","id":"label"}],"roughness":0,"endArrowhead":"arrow","updated":1},
        {"id":"label","type":"text","x":30,"y":10,"width":40,"height":20,"angle":0,"containerId":"arrow","fontSize":16,"fontFamily":5,"text":"Label","textAlign":"center","verticalAlign":"middle","lineHeight":1.25,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

#[test]
fn a_frame_charges_one_clip_mask_for_all_of_its_children() {
    let scene = SceneDocument::parse(FRAMED_RECTANGLES).expect("framed scene is admitted");
    let shared_total = CANVAS_PIXELS + FRAME_CLIP_PIXELS;

    let outcome = render_png(&scene, total_rasterized_limit(shared_total))
        .expect("the main surface plus one frame clip mask fits at exact equality");
    assert!(outcome.report().is_empty());

    let error = render_png(&scene, total_rasterized_limit(shared_total - 1))
        .expect_err("one pixel under the shared mask total is fatal");
    assert_resource_failure(
        &error,
        ErrorCode::RESOURCE_TOTAL_RASTERIZED_PIXELS_EXCEEDED,
        "max_total_rasterized_pixels",
        shared_total - 1,
        shared_total,
        "pixels",
        "/elements/0",
        Some("frame"),
    );
}

#[test]
fn a_frame_clip_does_not_count_against_a_childs_own_work_budget() {
    let scene = SceneDocument::parse(FRAMED_TEXT).expect("framed text scene is admitted");
    // A full-canvas alpha mask alone costs 8P work; a framed text element must
    // fit in that budget without one, or large boards silently lose their text.
    let one_canvas_mask = 8 * CANVAS_PIXELS;
    let limits = ResourceLimits::builder()
        .max_element_work_units(one_canvas_mask)
        .build()
        .expect("element work limit is valid");

    let outcome = render_png(&scene, limits).expect("framed text renders");

    assert!(
        outcome.report().is_empty(),
        "framed text was omitted: {:?}",
        outcome.report().diagnostics()
    );
}

#[test]
fn an_arrow_label_mask_is_bounded_by_the_label_rather_than_the_canvas() {
    let scene = SceneDocument::parse(LABELLED_ARROW).expect("labelled arrow scene is admitted");
    let canvas_pixels = 120 * 40;
    // Under a full-canvas label mask the main surface and the mask alone
    // already reach two canvases.
    let limits = ResourceLimits::builder()
        .max_raster_surface_pixels(canvas_pixels)
        .max_total_rasterized_pixels(2 * canvas_pixels - 1)
        .build()
        .expect("total rasterized limit is valid");

    let outcome = render_png(&scene, limits).expect("the label mask fits inside two canvases");

    assert!(outcome.report().is_empty());
}

/// The #112 reproduction at default limits: one 580x580 frame clipping a
/// 14x14 grid of filled rectangles whose last column and row straddle the frame
/// edge. With the frame name the canvas is 600x620 = 372,000 pixels, so a
/// full-canvas mask per clipped child would bill 197 x 372,000 = 73,284,000
/// rasterized pixels against the 67,108,864 default.
#[test]
fn a_frame_with_enough_children_to_exhaust_per_child_masks_renders_at_default_limits() {
    let mut elements = vec![json!({
        "id": "clip-frame", "type": "frame", "name": "Clip",
        "x": 0, "y": 0, "width": 580, "height": 580, "roughness": 0, "updated": 1
    })];
    for row in 0..14 {
        for column in 0..14 {
            elements.push(json!({
                "id": format!("cell-{row}-{column}"), "type": "rectangle",
                "x": 35 + 40 * column, "y": 35 + 40 * row, "width": 30, "height": 30,
                "frameId": "clip-frame", "strokeColor": "#1971c2", "backgroundColor": "#a5d8ff",
                "fillStyle": "solid", "roughness": 0, "updated": 1
            }));
        }
    }
    let scene = json!({
        "type": "excalidraw", "version": 2, "elements": elements, "appState": {}, "files": {}
    });
    let scene = SceneDocument::parse(&serde_json::to_vec(&scene).expect("scene serializes"))
        .expect("grid scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("default PNG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("197 framed elements render at default limits");

    assert!(
        outcome.report().is_empty(),
        "framed children were omitted: {:?}",
        outcome.report().diagnostics()
    );
    let (width, height) = png_dimensions(outcome.artifact().bytes());
    assert_eq!((width, height), (600, 620));
}

fn png_dimensions(png: &[u8]) -> (u32, u32) {
    let field = |offset: usize| u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap());
    (field(16), field(20))
}

fn total_rasterized_limit(total: u64) -> ResourceLimits {
    ResourceLimits::builder()
        .max_raster_surface_pixels(CANVAS_PIXELS)
        .max_total_rasterized_pixels(total)
        .build()
        .expect("resource limits are valid")
}

fn render_png(scene: &SceneDocument, limits: ResourceLimits) -> Result<RenderOutcome, RenderError> {
    let request = RenderRequest::builder(OutputFormat::Png)
        .frame_options(FrameOptions::default().with_name(false).with_outline(false))
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");
    Renderer::new().render(scene, &request)
}

#[allow(clippy::too_many_arguments)]
fn assert_resource_failure(
    error: &RenderError,
    code: ErrorCode,
    field: &str,
    configured: u64,
    attempted: u64,
    value_kind: &str,
    pointer: &str,
    element_id: Option<&str>,
) {
    assert_eq!(error.category(), RenderErrorCategory::ResourceLimit);
    assert_eq!(error.code(), code);
    assert_eq!(error.json_pointer(), pointer);
    assert_eq!(error.element_id(), element_id);
    assert!(error.report().is_empty());

    let descriptor = serde_json::to_value(error.descriptor()).expect("descriptor serializes");
    assert_eq!(
        descriptor["details"],
        json!({
            "kind": "resource_limit",
            "field": field,
            "configured": configured,
            "value_kind": value_kind,
            "measurement": {"kind": "attempted", "value": attempted},
            "recovery": null,
        })
    );
}
