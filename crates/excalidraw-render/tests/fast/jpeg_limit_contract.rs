//! JPEG limits (spec section 13.2): byte, dimension, pixel and work equality
//! and first crossing for baseline, progressive, CMYK, EXIF-rotated and
//! gain-map JPEGs in every format, with exact reports and recovery, and the
//! multi-crossing precedence: width before height before area, and the image
//! side before per-identity pixels. Dimensions are the oriented image's; a
//! gain map's secondary image counts toward bytes and work, not pixels.
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument};
use serde_json::{json, Value};

use crate::common::codec_fixtures::{jpeg_rgb, jpeg_with_app1_exif};

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const COLORS: [[u8; 3]; 4] = [[239, 31, 73], [19, 211, 97], [43, 67, 229], [251, 193, 17]];

fn scene(bytes: &[u8], mime: &str, width: u32, height: u32) -> Value {
    use base64::Engine as _;
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":width,"height":height,
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":mime,"created":1,
            "dataURL":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}}})
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

fn pixels(width: u32, height: u32) -> Vec<[u8; 3]> {
    (0..width * height)
        .map(|i| COLORS[(i % 4) as usize])
        .collect()
}

fn baseline(width: u32, height: u32) -> Vec<u8> {
    jpeg_rgb(width as u16, height as u16, &pixels(width, height))
}

fn progressive(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut bytes, 90);
    encoder.set_progressive(true);
    let rgb: Vec<u8> = pixels(width, height).into_iter().flatten().collect();
    encoder
        .encode(
            &rgb,
            width as u16,
            height as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .unwrap();
    bytes
}

fn cmyk(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let samples: Vec<u8> = pixels(width, height)
        .into_iter()
        .flat_map(|[r, g, b]| [255 - r, 255 - g, 255 - b, 0])
        .collect();
    jpeg_encoder::Encoder::new(&mut bytes, 90)
        .encode(
            &samples,
            width as u16,
            height as u16,
            jpeg_encoder::ColorType::Cmyk,
        )
        .unwrap();
    bytes
}

/// Stored `height`x`width`, drawn `width`x`height` through EXIF orientation 6.
fn rotated(width: u32, height: u32) -> Vec<u8> {
    jpeg_with_app1_exif(&baseline(height, width), 6)
}

/// A valid SDR-base MPF + hdrgm gain-map container from the frozen record.
fn gain_map() -> (Vec<u8>, u32, u32) {
    use base64::Engine as _;
    let record: Value = serde_json::from_slice(include_bytes!(
        "../../../../tests/oracle/jpeg-gain-map/reference.json"
    ))
    .unwrap();
    let row = record["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "baseline-little-sdr-ramp-ramp")
        .unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(row["jpeg_base64"].as_str().unwrap())
        .unwrap();
    (
        bytes,
        row["width"].as_u64().unwrap() as u32,
        row["height"].as_u64().unwrap() as u32,
    )
}

type Source = (&'static str, fn(u32, u32) -> Vec<u8>);
const SOURCES: [Source; 4] = [
    ("baseline", baseline),
    ("progressive", progressive),
    ("cmyk", cmyk),
    ("rotated", rotated),
];

/// Equality passes silently and the first crossing reports exactly the
/// attempted total, and recovers like a missing asset. Direct SVG validates
/// without deriving pixels.
#[test]
pub(super) fn jpeg_limits_have_exact_equality_and_first_crossing() {
    let (map, map_width, map_height) = gain_map();
    let mut sources: Vec<_> = SOURCES
        .iter()
        .map(|&(name, build)| (name, build(9, 7), 9, 7))
        .collect();
    sources.push(("gain-map", map, map_width, map_height));
    for (name, bytes, width, height) in sources {
        let source = scene(&bytes, "image/jpeg", width, height);
        for format in FORMATS {
            for field in [
                "max_asset_bytes",
                "max_raster_dimension",
                "max_decoded_pixels_per_asset",
                "max_asset_work_units",
            ] {
                let (_, probe) = render(&source, format, &limits(field, 1));
                if format == OutputFormat::Svg
                    && matches!(
                        field,
                        "max_raster_dimension" | "max_decoded_pixels_per_asset"
                    )
                {
                    assert_eq!(probe["diagnostics"], json!([]), "{name} {field} {format:?}");
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
                assert_eq!(
                    at_equality["diagnostics"],
                    json!([]),
                    "{name} {field} {format:?} equality {attempted}"
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
pub(super) fn jpeg_raster_crossings_order_width_then_height_then_area() {
    for (name, build) in SOURCES {
        for (width, height, dimension, pixels, expected) in CASES {
            let source = scene(&build(width, height), "image/jpeg", width, height);
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
                let context =
                    format!("{name} {width}x{height} {dimension:?}/{pixels:?} {format:?}");
                let expected = match expected.filter(|_| format != OutputFormat::Svg) {
                    Some((field, attempted)) => {
                        let configured = if field == "max_raster_dimension" {
                            u64::from(dimension.unwrap())
                        } else {
                            pixels.unwrap()
                        };
                        json!([crossing(format, field, configured, attempted)])
                    }
                    None => json!([]),
                };
                assert_eq!(report["diagnostics"], expected, "{context}");
            }
        }
    }
}

/// JPEG limits facet: equality and first crossing for every limit and shape,
/// and the multi-crossing precedence.
#[test]
fn jpeg_limits_have_exact_boundaries_and_precedence() {
    jpeg_limits_have_exact_equality_and_first_crossing();
    jpeg_raster_crossings_order_width_then_height_then_area();
}
