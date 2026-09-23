//! BMP and ICO limits (spec section 13.2): byte, dimension, pixel and work
//! equality and first crossing for direct, palette RLE, DIB icon and PNG icon
//! payloads in every format, with exact reports and recovery, and the
//! multi-crossing precedence: width before height before area, and the image
//! side before per-identity pixels. An icon's image is its selected entry.
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument};
use serde_json::{json, Value};

use crate::common::codec_fixtures::{bmp24, ico, png_rgba};

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

/// A bottom-up eight-bit `BI_RLE8` BMP whose rows are literal runs over a
/// four-entry palette, ended by end-of-line and end-of-bitmap codes.
fn bmp_rle8(width: u32, height: u32) -> Vec<u8> {
    let mut encoded = Vec::new();
    for y in 0..height {
        for x in (0..width).step_by(255) {
            let run = (width - x).min(255) as u8;
            encoded.extend_from_slice(&[run, ((x + y) % 4) as u8]);
        }
        encoded.extend_from_slice(&[0, 0]);
    }
    encoded.extend_from_slice(&[0, 1]);
    let offset = 14 + 40 + 16_u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(offset + encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&offset.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&(width as i32).to_le_bytes());
    bytes.extend_from_slice(&(height as i32).to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    for [red, green, blue] in COLORS {
        bytes.extend_from_slice(&[blue, green, red, 0]);
    }
    bytes.extend_from_slice(&encoded);
    bytes
}

/// A 32-bit icon DIB of `width`x`height` with partial alpha and its AND mask.
fn dib32(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&(width as i32).to_le_bytes());
    bytes.extend_from_slice(&(2 * height as i32).to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 24]);
    for (i, [red, green, blue]) in pixels(width, height).into_iter().enumerate() {
        bytes.extend_from_slice(&[blue, green, red, [255, 128, 0, 64][i % 4]]);
    }
    let mask_row = width.div_ceil(32) as usize * 4;
    bytes.resize(bytes.len() + mask_row * height as usize, 0);
    bytes
}

/// An icon whose selected (largest) entry is a `width`x`height` DIB, after a
/// smaller entry.
fn ico_dib(width: u32, height: u32) -> Vec<u8> {
    ico(&[
        (1, 1, 32, dib32(1, 1)),
        (width as u16, height as u16, 32, dib32(width, height)),
    ])
}

/// An icon whose one entry is a `width`x`height` PNG.
fn ico_png(width: u32, height: u32) -> Vec<u8> {
    let rgba: Vec<[u8; 4]> = pixels(width, height)
        .into_iter()
        .map(|[r, g, b]| [r, g, b, 255])
        .collect();
    ico(&[(
        width as u16,
        height as u16,
        32,
        png_rgba(width, height, &rgba),
    )])
}

type Source = (&'static str, &'static str, fn(u32, u32) -> Vec<u8>);
const SOURCES: [Source; 4] = [
    ("bmp", "image/bmp", |w, h| bmp24(w, h, &pixels(w, h))),
    ("bmp-rle8", "image/bmp", bmp_rle8),
    ("ico-dib", "image/x-icon", ico_dib),
    ("ico-png", "image/x-icon", ico_png),
];

/// Equality passes silently and the first crossing reports exactly the
/// attempted total, and recovers like a missing asset. Direct SVG validates
/// without deriving pixels.
fn exact_equality_and_first_crossing(family: &str) {
    for (name, mime, build) in SOURCES.into_iter().filter(|s| s.0.starts_with(family)) {
        let source = scene(&build(9, 7), mime, 9, 7);
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
fn raster_crossings_order_width_then_height_then_area(family: &str) {
    for (name, mime, build) in SOURCES.into_iter().filter(|s| s.0.starts_with(family)) {
        for (width, height, dimension, pixels, expected) in CASES {
            let source = scene(&build(width, height), mime, width, height);
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

/// BMP limits facet: equality and first crossing for every limit and shape,
/// and the multi-crossing precedence.
#[test]
fn bmp_limits_have_exact_boundaries_and_precedence() {
    exact_equality_and_first_crossing("bmp");
    raster_crossings_order_width_then_height_then_area("bmp");
}

/// ICO limits facet: equality and first crossing for DIB and PNG icons, the
/// selected entry's dimensions and pixels, and the multi-crossing precedence.
#[test]
fn ico_limits_have_exact_boundaries_and_precedence() {
    exact_equality_and_first_crossing("ico");
    raster_crossings_order_width_then_height_then_area("ico");
}
