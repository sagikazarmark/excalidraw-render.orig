//! PNG paints an axis-aligned image by pixel-centre coverage, as the pinned
//! Chromium canvas's `drawImage` does for a destination whose transform keeps
//! rectangles rectangles (#162): Skia rounds each destination edge to
//! `floor(edge + 0.5)`, so a device pixel is painted if and only if its centre
//! lies inside the destination rectangle — a centre on the left or top edge
//! falls outside, one on the right or bottom edge inside — and a fractional
//! edge is a whole row or column or none, never a blend. Sampling keeps the
//! exact fractional mapping. A rotated image, the rounded-corner clip and the
//! frame clips stay antialiased, as the canvas antialiases them. A nested-SVG
//! image is drawn as the canvas draws an SVG image (Blink
//! `SVGImage::DrawInternal`): its content and its viewport clip are both
//! antialiased, so a fractional edge carries its coverage squared, inside a
//! non-antialiased clip of the element rectangle enlarged to whole
//! element-space pixels and snapped to device pixels.

use base64::Engine;
use excalidraw_render::{OutputFormat, RenderRequest, SceneDocument, Sizing};

use crate::common::raster::{decode_png, DecodedPng};
use crate::common::render::{artifact_without_diagnostics, render_with_request};

const RED: [u8; 4] = [255, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// A scene of a 4x4 solid red PNG image completed by `element_fields`, beside
/// a 4x4 fully transparent PNG image at the origin that paints nothing and
/// pins the scene origin to (0, 0), so a fractional element position is a
/// fractional device position rather than the scene's own origin.
fn red_image_scene(element_fields: &str) -> SceneDocument {
    let red = base64::engine::general_purpose::STANDARD.encode(fixture_png(4, 4, &[RED; 16]));
    let clear = base64::engine::general_purpose::STANDARD.encode(fixture_png(4, 4, &[CLEAR; 16]));
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"origin","type":"image","fileId":"clear","status":"saved","seed":1,"updated":1,
                 "x":0,"y":0,"width":4,"height":4,"angle":0,"opacity":100,"scale":[1,1]}},
                {{"id":"image","type":"image","fileId":"red","status":"saved","seed":2,"updated":1,{element_fields}}}
            ],
            "appState":{{}},
            "files":{{
                "red":{{"id":"red","mimeType":"image/png","dataURL":"data:image/png;base64,{red}","created":1}},
                "clear":{{"id":"clear","mimeType":"image/png","dataURL":"data:image/png;base64,{clear}","created":1}}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}

fn render_png(scene: &SceneDocument, sizing: Sizing) -> DecodedPng {
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(false)
        .sizing(sizing)
        .build()
        .expect("request is valid");
    decode_png(&artifact_without_diagnostics(&render_with_request(
        scene, &request,
    )))
}

/// Whether a pixel is a blend of the red image over the clear background:
/// neither fully painted nor untouched.
fn is_blended(pixel: [u8; 4]) -> bool {
    pixel != RED && pixel != CLEAR
}

#[test]
fn a_fractional_position_paints_whole_rows_and_columns_by_pixel_centre_coverage() {
    // The image spans device [10.25, 30.25) x [10.5, 30.5) on a 41x41 surface
    // (bounds 20.25x20 plus 10 px padding, truncated). Column 10 (centre 10.5,
    // right of 10.25) is painted whole and column 30 (centre 30.5, past
    // 30.25) is empty; row 10 (centre 10.5, on the top edge) is empty and row
    // 30 (centre 30.5, on the bottom edge) is painted whole.
    let scene = red_image_scene(
        r#""x":0.25,"y":0.5,"width":20,"height":20,"angle":0,"opacity":100,"scale":[1,1]"#,
    );
    let png = render_png(&scene, Sizing::Scale(1.0));
    assert_eq!(
        png.pixel(10, 20),
        RED,
        "left edge column, centre right of the edge"
    );
    assert_eq!(png.pixel(9, 20), CLEAR);
    assert_eq!(png.pixel(29, 20), RED);
    assert_eq!(
        png.pixel(30, 20),
        CLEAR,
        "right edge column, centre past the edge"
    );
    assert_eq!(png.pixel(20, 10), CLEAR, "top edge row, centre on the edge");
    assert_eq!(png.pixel(20, 11), RED);
    assert_eq!(
        png.pixel(20, 30),
        RED,
        "bottom edge row, centre on the edge"
    );
    assert_eq!(png.pixel(20, 31), CLEAR);
    for y in 0..png.height as usize {
        for x in 0..png.width as usize {
            assert!(
                !is_blended(png.pixel(x, y)),
                "({x}, {y}) is a blended edge pixel: {:?}",
                png.pixel(x, y)
            );
        }
    }
}

#[test]
fn a_fractional_size_snaps_in_device_pixels_at_every_export_scale() {
    // 96.5 x 63.25 at the origin: at scale 1 the image spans device
    // [10, 106.5) x [10, 73.25), so column 106 is whole and row 73 empty; at
    // scale 2 it spans [20, 213) x [20, 146.5), so column 212 is whole, 213
    // empty, row 146 whole and 147 empty. The rule is applied in device
    // pixels, where the canvas's transform places the rectangle.
    let scene = red_image_scene(
        r#""x":0,"y":0,"width":96.5,"height":63.25,"angle":0,"opacity":100,"scale":[1,1]"#,
    );
    let png = render_png(&scene, Sizing::Scale(1.0));
    assert_eq!(png.pixel(106, 40), RED);
    assert_eq!(png.pixel(107, 40), CLEAR);
    assert_eq!(png.pixel(50, 72), RED);
    assert_eq!(png.pixel(50, 73), CLEAR);

    let png = render_png(&scene, Sizing::Scale(2.0));
    assert_eq!(png.pixel(212, 80), RED);
    assert_eq!(png.pixel(213, 80), CLEAR);
    assert_eq!(png.pixel(100, 146), RED);
    assert_eq!(png.pixel(100, 147), CLEAR);
    for (x, y) in [
        (212, 80),
        (213, 80),
        (100, 146),
        (100, 147),
        (20, 20),
        (19, 19),
    ] {
        assert!(!is_blended(png.pixel(x, y)), "({x}, {y}) is blended");
    }
}

#[test]
fn a_flipped_fractional_image_snaps_the_same_rectangle() {
    // The flip mirrors the content about the element's centre and leaves the
    // destination rectangle where it was, so the same columns are whole.
    let scene = red_image_scene(
        r#""x":0.25,"y":0.5,"width":20,"height":20,"angle":0,"opacity":100,"scale":[-1,-1]"#,
    );
    let png = render_png(&scene, Sizing::Scale(1.0));
    assert_eq!(png.pixel(10, 20), RED);
    assert_eq!(png.pixel(30, 20), CLEAR);
    assert_eq!(png.pixel(20, 10), CLEAR);
    assert_eq!(png.pixel(20, 30), RED);
}

#[test]
fn a_rotated_image_keeps_antialiased_edges() {
    // At 30 degrees the transform no longer keeps rectangles rectangles, so
    // the canvas antialiases the edges and so does PNG: the pixels along a
    // slanted edge are blends.
    let scene = red_image_scene(
        r#""x":0,"y":0,"width":40,"height":40,"angle":0.5235987755982988,"opacity":100,"scale":[1,1]"#,
    );
    let png = render_png(&scene, Sizing::Scale(1.0));
    let blended = (0..png.height as usize)
        .flat_map(|y| (0..png.width as usize).map(move |x| (x, y)))
        .filter(|&(x, y)| is_blended(png.pixel(x, y)))
        .count();
    assert!(
        blended > 40,
        "a rotated image's edges are antialiased: {blended} blended pixels"
    );
}

#[test]
fn the_rounded_corner_clip_stays_antialiased_while_the_straight_edges_snap() {
    // A rounded image whose right edge lies at 106.5: the straight edge column
    // 106 is painted whole where the corner arcs do not reach it, while the
    // arcs themselves are antialiased as the canvas's `roundRect` clip is.
    let scene = red_image_scene(
        r#""x":0,"y":0,"width":96.5,"height":64,"angle":0,"opacity":100,"scale":[1,1],"roundness":{"type":3}"#,
    );
    let png = render_png(&scene, Sizing::Scale(1.0));
    // The clip at 106.5 halves the snapped column: the canvas paints the
    // column whole and its antialiased clip attenuates it by half.
    let edge = png.pixel(106, 42);
    assert!(
        (64..=192).contains(&edge[3]) && edge[0] == 255,
        "the rounded clip's straight edge is half covered at the fractional edge: {edge:?}"
    );
    assert_eq!(png.pixel(105, 42), RED);
    assert_eq!(png.pixel(107, 42), CLEAR);
    // A corner arc: the radius is min(32, 16) = 16, so the pixel just inside
    // the corner square is clear and the arc's pixels are blends.
    assert_eq!(png.pixel(10, 10), CLEAR);
    let arc = (10..26usize)
        .flat_map(|y| (10..26usize).map(move |x| (x, y)))
        .filter(|&(x, y)| is_blended(png.pixel(x, y)))
        .count();
    assert!(
        arc > 8,
        "the corner arc is antialiased: {arc} blended pixels"
    );
}

#[test]
fn a_nested_svg_at_a_fractional_position_squares_its_edge_coverage_inside_the_snapped_clip() {
    // A 96x96 nested SVG at (0.25, 0.5) spans device [10.25, 106.25) x
    // [10.5, 106.5) once padded and pinned to the origin. The official canvas
    // (measured on the pinned reference profile for #162) paints the
    // quarter-covered left column at 0.75 * 0.75 of the content and the
    // half-covered bottom row at 0.5 * 0.5, and nothing in the right column or
    // the top row: `clipRect(ToEnclosingRect(0 0 96 96))` snaps to device
    // [10, 106) x [11, 107) and drops them before the squared coverage could
    // show.
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32"><rect width="32" height="32" fill="#ff0000"/></svg>"##;
    let clear = base64::engine::general_purpose::STANDARD.encode(fixture_png(4, 4, &[CLEAR; 16]));
    let svg = base64::engine::general_purpose::STANDARD.encode(nested);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"origin","type":"image","fileId":"clear","status":"saved","seed":1,"updated":1,
                 "x":0,"y":0,"width":4,"height":4,"angle":0,"opacity":100,"scale":[1,1]}},
                {{"id":"image","type":"image","fileId":"svg","status":"saved","seed":2,"updated":1,
                 "x":0.25,"y":0.5,"width":96,"height":96,"angle":0,"opacity":100,"scale":[1,1]}}
            ],
            "appState":{{}},
            "files":{{
                "svg":{{"id":"svg","mimeType":"image/svg+xml","dataURL":"data:image/svg+xml;base64,{svg}","created":1}},
                "clear":{{"id":"clear","mimeType":"image/png","dataURL":"data:image/png;base64,{clear}","created":1}}
            }}
        }}"#
    );
    let scene = SceneDocument::parse(json.as_bytes()).expect("nested SVG scene is admitted");
    let png = render_png(&scene, Sizing::Scale(1.0));
    assert_eq!((png.width, png.height), (116, 116));
    let alpha = |x: usize, y: usize| png.pixel(x, y)[3];
    assert_eq!(png.pixel(50, 50), RED);
    // Left edge at 10.25: coverage 0.75, squared 0.5625 -> alpha 143 or 144.
    assert!(
        (142..=145).contains(&alpha(10, 50)),
        "left column: {:?}",
        png.pixel(10, 50)
    );
    assert_eq!(alpha(9, 50), 0);
    // Right edge at 106.25: the snapped enclosing clip ends at 106.
    assert_eq!(alpha(105, 50), 255);
    assert_eq!(
        alpha(106, 50),
        0,
        "column 106 lies outside the snapped clip"
    );
    // Top edge at 10.5: the snapped clip starts at 11.
    assert_eq!(alpha(50, 10), 0, "row 10 lies outside the snapped clip");
    assert_eq!(alpha(50, 11), 255);
    // Bottom edge at 106.5: coverage 0.5, squared 0.25 -> alpha 64.
    assert!(
        (62..=66).contains(&alpha(50, 106)),
        "bottom row: {:?}",
        png.pixel(50, 106)
    );
    assert_eq!(alpha(50, 107), 0);
}

fn fixture_png(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("fixture PNG header encodes");
        let rgba: Vec<u8> = pixels.iter().flatten().copied().collect();
        writer
            .write_image_data(&rgba)
            .expect("fixture PNG pixels encode");
    }
    bytes
}
