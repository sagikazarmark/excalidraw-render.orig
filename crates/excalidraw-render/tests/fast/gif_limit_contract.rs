//! GIF limits (spec section 13.2): byte, dimension, pixel and work equality
//! and first crossing for still, transparent and animated GIF in every
//! format, with exact reports and recovery, and the multi-crossing
//! precedence: width before height before area, and the image side before
//! per-identity pixels. The canvas is the logical screen grown to the first
//! frame, as the reference reads it.
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument};
use serde_json::{json, Value};

use crate::common::codec_fixtures::{gif, gif_frame, gif_with_screen};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

fn scene(bytes: &[u8], width: u32, height: u32) -> Value {
    use base64::Engine as _;
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":width,"height":height,
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/gif","created":1,
            "dataURL":format!("data:image/gif;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}}})
}

/// Renders twice, requires repetition, and returns the artifact and the report
/// without messages.
fn render(source: &Value, format: OutputFormat, limits: &ResourceLimits) -> (Vec<u8>, Value) {
    let scene = SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap();
    let request = RenderRequest::builder(format)
        .padding(0.0)
        .include_background(false)
        .resource_limits(limits.clone())
        .build()
        .unwrap();
    let first = Renderer::new().render(&scene, &request).unwrap();
    let second = Renderer::new().render(&scene, &request).unwrap();
    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    let mut report = serde_json::to_value(first.report()).unwrap();
    for d in report["diagnostics"].as_array_mut().unwrap() {
        assert!(!d["message"].as_str().unwrap().is_empty());
        d.as_object_mut().unwrap().remove("message");
    }
    (first.artifact().bytes().to_vec(), report)
}

fn limits(field: &str, value: u64) -> ResourceLimits {
    let builder = ResourceLimits::builder();
    match field {
        "max_asset_bytes" => builder.max_asset_bytes(value),
        "max_raster_dimension" => builder.max_raster_dimension(value as u32),
        "max_decoded_pixels_per_asset" => builder.max_decoded_pixels_per_asset(value),
        "max_asset_work_units" => builder.max_asset_work_units(value),
        _ => unreachable!(),
    }
    .build()
    .unwrap()
}

fn crossing(format: OutputFormat, field: &str, configured: u64, attempted: u64) -> Value {
    let (code, kind, element) = match field {
        "max_asset_bytes" => ("resource.asset-bytes-exceeded", "bytes", json!("image")),
        "max_raster_dimension" => (
            "resource.image-dimensions-exceeded",
            "dimension",
            Value::Null,
        ),
        "max_decoded_pixels_per_asset" => ("resource.image-pixels-exceeded", "pixels", Value::Null),
        "max_asset_work_units" => ("resource.asset-work-exceeded", "work_units", json!("image")),
        _ => unreachable!(),
    };
    json!({"code":code,"severity":"warning","format":format.as_str(),"effects":["visual"],"element_id":element,
        "json_pointer":"/files/asset/dataURL","details":{"kind":"resource_limit","field":field,"configured":configured,
        "value_kind":kind,"measurement":{"kind":"attempted","value":attempted},
        "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"}}})
}

const PALETTE: [u8; 12] = [239, 31, 73, 19, 211, 97, 43, 67, 229, 251, 193, 17];

fn still(width: u16, height: u16) -> Vec<u8> {
    let indices: Vec<u8> = (0..width * height).map(|i| (i % 4) as u8).collect();
    gif(&[gif_frame(&indices, width, height, &PALETTE, None)])
}

fn animation(width: u16, height: u16) -> Vec<u8> {
    let first: Vec<u8> = (0..width * height).map(|i| (i % 4) as u8).collect();
    let later = vec![1_u8; 15];
    gif_with_screen(
        width,
        height,
        &[],
        None,
        &[
            gif_frame(&first, width, height, &PALETTE, None),
            gif_frame(&later, 5, 3, &PALETTE, None),
            gif_frame(&first, width, height, &PALETTE, None),
        ],
    )
}

fn sources() -> Vec<(&'static str, Vec<u8>, u32, u32)> {
    let transparent: Vec<u8> = (0..63).map(|i| (i % 4) as u8).collect();
    vec![
        ("still", still(9, 7), 9, 7),
        (
            "transparent",
            gif(&[gif_frame(&transparent, 9, 7, &PALETTE, Some(2))]),
            9,
            7,
        ),
        ("animated", animation(9, 7), 9, 7),
    ]
}

/// Equality passes silently and the first crossing reports exactly the
/// attempted total, and recovers like a missing asset. Direct SVG validates
/// without deriving pixels.
#[test]
pub(super) fn gif_limits_have_exact_equality_and_first_crossing() {
    for (name, bytes, width, height) in sources() {
        let source = scene(&bytes, width, height);
        let animated = name == "animated";
        for format in FORMATS {
            for field in [
                "max_asset_bytes",
                "max_raster_dimension",
                "max_decoded_pixels_per_asset",
                "max_asset_work_units",
            ] {
                let silent_in_svg = format == OutputFormat::Svg
                    && matches!(
                        field,
                        "max_raster_dimension" | "max_decoded_pixels_per_asset"
                    );
                let (_, probe) = render(&source, format, &limits(field, 1));
                if silent_in_svg {
                    assert!(
                        probe["diagnostics"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|d| d["code"] == "image.animation-flattened"),
                        "{name} {field} {format:?}"
                    );
                    continue;
                }
                // Charges accrue in phases; raise the limit to each attempt
                // until the render fits, which is the exact total.
                let attempted_at = |report: &Value| {
                    report["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|d| d["details"]["field"] == field)
                        .map(|d| d["details"]["measurement"]["value"].as_u64().unwrap())
                };
                let mut attempted = attempted_at(&probe).unwrap_or_else(|| {
                    panic!("{name} {field} {format:?}: no crossing at 1: {probe}")
                });
                while let Some(next) =
                    attempted_at(&render(&source, format, &limits(field, attempted)).1)
                {
                    assert!(
                        next > attempted,
                        "{name} {field} {format:?}: charges must grow"
                    );
                    attempted = next;
                }
                let (_, at_equality) = render(&source, format, &limits(field, attempted));
                let flattened = animated && format != OutputFormat::Svg;
                assert_eq!(
                    at_equality["diagnostics"].as_array().unwrap().len(),
                    usize::from(flattened),
                    "{name} {field} {format:?} equality {attempted}: {at_equality}"
                );
                let (crossed, report) = render(&source, format, &limits(field, attempted - 1));
                assert_eq!(
                    report,
                    json!({"schema_version":1,"diagnostics":[crossing(format, field, attempted - 1, attempted)]}),
                    "{name} {field} {format:?}"
                );
                let mut missing = source.clone();
                missing["files"] = json!({});
                assert_eq!(
                    crossed,
                    render(&missing, format, &limits(field, attempted - 1)).0,
                    "{name} {field} {format:?} recovery"
                );
            }
        }
    }
}

type Case = (
    u32,
    u32,
    Option<u32>,
    Option<u64>,
    Option<(&'static str, u64)>,
);
const CASES: [Case; 9] = [
    (5, 7, Some(4), None, Some(("max_raster_dimension", 5))),
    (7, 5, Some(4), None, Some(("max_raster_dimension", 7))),
    (4, 7, Some(4), None, Some(("max_raster_dimension", 7))),
    (7, 4, Some(4), None, Some(("max_raster_dimension", 7))),
    (4, 4, Some(4), None, None),
    (5, 7, Some(4), Some(10), Some(("max_raster_dimension", 5))),
    (
        4,
        7,
        Some(7),
        Some(27),
        Some(("max_decoded_pixels_per_asset", 28)),
    ),
    (4, 7, Some(7), Some(28), None),
    (
        9,
        9,
        Some(9),
        Some(80),
        Some(("max_decoded_pixels_per_asset", 81)),
    ),
];

/// Width precedes height, then area; the image side precedes pixels.
#[test]
pub(super) fn gif_raster_crossings_order_width_then_height_then_area() {
    for animated in [false, true] {
        for (width, height, dimension, pixels, expected) in CASES {
            let (w, h) = (width as u16, height as u16);
            let bytes = if animated {
                animation(w, h)
            } else {
                still(w, h)
            };
            let source = scene(&bytes, width, height);
            let mut builder = ResourceLimits::builder();
            if let Some(dimension) = dimension {
                builder = builder.max_raster_dimension(dimension);
            }
            if let Some(pixels) = pixels {
                builder = builder.max_decoded_pixels_per_asset(pixels);
            }
            let limits = builder.build().unwrap();
            for format in FORMATS {
                let (_, report) = render(&source, format, &limits);
                let context = format!(
                    "{width}x{height} animated={animated} {dimension:?}/{pixels:?} {format:?}"
                );
                let expected = match expected.filter(|_| format != OutputFormat::Svg) {
                    Some((field, attempted)) => {
                        let configured = if field == "max_raster_dimension" {
                            u64::from(dimension.unwrap())
                        } else {
                            pixels.unwrap()
                        };
                        json!([crossing(format, field, configured, attempted)])
                    }
                    None if animated && format != OutputFormat::Svg => {
                        json!([{"code":"image.animation-flattened","severity":"info","format":format.as_str(),"effects":["visual"],"element_id":"image","json_pointer":"/elements/0/fileId","details":{"kind":"asset","asset_id":"asset","claimed_media_type":null,"validated_media_type":"image/gif","recovery":"flattened","recipe_id":null}}])
                    }
                    None => json!([]),
                };
                assert_eq!(report["diagnostics"], expected, "{context}");
            }
        }
    }
}

/// GIF limits facet: equality and first crossing for every limit and shape,
/// and the multi-crossing precedence.
#[test]
fn gif_limits_have_exact_boundaries_and_precedence() {
    gif_limits_have_exact_equality_and_first_crossing();
    gif_raster_crossings_order_width_then_height_then_area();
}
