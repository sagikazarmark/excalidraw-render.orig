//! PNG multi-crossing precedence (spec section 13.2): for multiple raster
//! crossings width precedes height, then area, and the local image-side
//! crossing precedes the per-identity pixel crossing. Exercised over 8-bit,
//! 16-bit and Adam7-interlaced sources in every format, at equality and first
//! crossing, with exact reports and recovery.
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, ResourceLimits, SceneDocument};
use serde_json::{json, Value};

/// A solid RGBA PNG; Adam7 writes each pass as its own reduced image.
fn png(width: u32, height: u32, depth: u8, adam7: bool) -> Vec<u8> {
    let pixel: Vec<u8> = if depth == 16 {
        vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xff, 0xff]
    } else {
        vec![0x12, 0x56, 0x9a, 0xff]
    };
    let passes: &[(u32, u32, u32, u32)] = if adam7 {
        &[
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ]
    } else {
        &[(0, 0, 1, 1)]
    };
    let mut raw = Vec::new();
    for &(x0, y0, dx, dy) in passes {
        let columns = (width + dx - 1 - x0.min(width)) / dx;
        let rows = (height + dy - 1 - y0.min(height)) / dy;
        if x0 >= width || y0 >= height || columns == 0 || rows == 0 {
            continue;
        }
        // Each scanline is filter type 0 followed by its solid pixels.
        let scanline: Vec<u8> = std::iter::once(0)
            .chain(
                pixel
                    .iter()
                    .copied()
                    .cycle()
                    .take(pixel.len() * columns as usize),
            )
            .collect();
        for _ in 0..rows {
            raw.extend_from_slice(&scanline);
        }
    }
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[depth, 6, 0, 0, u8::from(adam7)]);
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [
        (*b"IHDR", ihdr),
        (
            *b"IDAT",
            miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
        ),
        (*b"IEND", Vec::new()),
    ] {
        crate::common::codec_fixtures::append_png_chunk(&mut bytes, kind, &data);
    }
    bytes
}

fn scene(bytes: &[u8], width: u32, height: u32) -> Value {
    use base64::Engine as _;
    json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":width,"height":height,
        "fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/png","created":1,
            "dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}}})
}

fn render(
    source: &Value,
    format: OutputFormat,
    limits: &ResourceLimits,
) -> excalidraw_render::RenderOutcome {
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
    first
}

/// `(width, height, max_raster_dimension, max_decoded_pixels_per_asset,
/// expected crossing)` with the crossing as `(field, value kind, attempted)`.
type Case = (
    u32,
    u32,
    Option<u32>,
    Option<u64>,
    Option<(&'static str, &'static str, u64)>,
);
const CASES: [Case; 9] = [
    (
        5,
        7,
        Some(4),
        None,
        Some(("max_raster_dimension", "dimension", 5)),
    ),
    (
        7,
        5,
        Some(4),
        None,
        Some(("max_raster_dimension", "dimension", 7)),
    ),
    (
        4,
        7,
        Some(4),
        None,
        Some(("max_raster_dimension", "dimension", 7)),
    ),
    (
        7,
        4,
        Some(4),
        None,
        Some(("max_raster_dimension", "dimension", 7)),
    ),
    (4, 4, Some(4), None, None),
    (
        5,
        7,
        Some(4),
        Some(10),
        Some(("max_raster_dimension", "dimension", 5)),
    ),
    (
        4,
        7,
        Some(7),
        Some(27),
        Some(("max_decoded_pixels_per_asset", "pixels", 28)),
    ),
    (4, 7, Some(7), Some(28), None),
    (
        9,
        9,
        Some(9),
        Some(80),
        Some(("max_decoded_pixels_per_asset", "pixels", 81)),
    ),
];

#[test]
pub(super) fn raster_crossings_order_width_then_height_then_area() {
    for (depth, adam7) in [(8, false), (16, false), (8, true)] {
        for (width, height, dimension, pixels, crossing) in CASES {
            let source = scene(&png(width, height, depth, adam7), width, height);
            let mut limits = ResourceLimits::builder();
            if let Some(dimension) = dimension {
                limits = limits.max_raster_dimension(dimension);
            }
            if let Some(pixels) = pixels {
                limits = limits.max_decoded_pixels_per_asset(pixels);
            }
            let limits = limits.build().unwrap();
            let context =
                format!("{width}x{height} d{depth} adam7={adam7} {dimension:?}/{pixels:?}");
            for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
                let outcome = render(&source, format, &limits);
                let mut actual = serde_json::to_value(outcome.report()).unwrap();
                for r in actual["diagnostics"].as_array_mut().unwrap() {
                    assert!(!r["message"].as_str().unwrap().is_empty());
                    r.as_object_mut().unwrap().remove("message");
                }
                // Direct SVG embeds the validated payload without deriving pixels.
                let Some((field, kind, attempted)) =
                    crossing.filter(|_| format != OutputFormat::Svg)
                else {
                    assert_eq!(
                        actual,
                        json!({"schema_version":1,"diagnostics":[]}),
                        "{context} {format:?}"
                    );
                    continue;
                };
                let configured = if kind == "dimension" {
                    u64::from(dimension.unwrap())
                } else {
                    pixels.unwrap()
                };
                let code = if kind == "dimension" {
                    "resource.image-dimensions-exceeded"
                } else {
                    "resource.image-pixels-exceeded"
                };
                assert_eq!(
                    actual,
                    json!({"schema_version":1,"diagnostics":[{"code":code,"severity":"warning","format":format.as_str(),
                        "effects":["visual"],"element_id":null,"json_pointer":"/files/asset/dataURL",
                        "details":{"kind":"resource_limit","field":field,"configured":configured,"value_kind":kind,
                            "measurement":{"kind":"attempted","value":attempted},
                            "recovery":if format == OutputFormat::Png {"placeholder"} else {"omitted"}}}]}),
                    "{context} {format:?}"
                );
                // Recovery is the same as an absent asset, under the same limits.
                let mut removed = source.clone();
                removed["files"] = json!({});
                assert_eq!(
                    outcome.artifact().bytes(),
                    render(&removed, format, &limits).artifact().bytes(),
                    "{context} {format:?} recovery"
                );
            }
        }
    }
}
