//! PNG resamples an axis-aligned raster image once per destination pixel
//! centre, as the pinned Chromium canvas's `drawImage` does (#165): a bilinear
//! sample at each device pixel's centre under the exact fractional mapping,
//! with four-bit fractional weights and a truncated premultiplied sum, painted
//! as one constant block of subpixels, so an upscaled image's texel
//! boundaries fall where the canvas puts them and a 1:1 image is its own
//! texels. Sampling the image at every subpixel centre of the fixed
//! supersample and box-filtering instead blurred a 1:1 image by
//! `[1/8, 3/4, 1/8]` across every texel boundary and gave an upscaled image a
//! footprint one destination pixel wider than the canvas's.

use base64::Engine;
use excalidraw_render::{OutputFormat, RenderRequest, SceneDocument, Sizing};

use crate::common::raster::{decode_png, DecodedPng};
use crate::common::render::{artifact_without_diagnostics, render_with_request};

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// A scene of a 2x2 checker PNG (black top-left and bottom-right, white
/// elsewhere) placed by `element_fields`, beside a 4x4 fully transparent PNG at
/// the origin that pins the scene origin to (0, 0).
fn checker_scene(element_fields: &str) -> SceneDocument {
    let checker = base64::engine::general_purpose::STANDARD.encode(fixture_png(
        2,
        2,
        &[BLACK, WHITE, WHITE, BLACK],
    ));
    let clear = base64::engine::general_purpose::STANDARD.encode(fixture_png(4, 4, &[CLEAR; 16]));
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"origin","type":"image","fileId":"clear","status":"saved","seed":1,"updated":1,
                 "x":0,"y":0,"width":4,"height":4,"angle":0,"opacity":100,"scale":[1,1]}},
                {{"id":"image","type":"image","fileId":"checker","status":"saved","seed":2,"updated":1,{element_fields}}}
            ],
            "appState":{{}},
            "files":{{
                "checker":{{"id":"checker","mimeType":"image/png","dataURL":"data:image/png;base64,{checker}","created":1}},
                "clear":{{"id":"clear","mimeType":"image/png","dataURL":"data:image/png;base64,{clear}","created":1}}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("checker scene is admitted")
}

fn render_png(scene: &SceneDocument) -> DecodedPng {
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(false)
        .sizing(Sizing::Scale(1.0))
        .build()
        .expect("request is valid");
    decode_png(&artifact_without_diagnostics(&render_with_request(
        scene, &request,
    )))
}

/// The grey level of an opaque pixel whose channels agree.
fn level(pixel: [u8; 4]) -> u8 {
    assert_eq!(pixel[3], 255, "opaque pixel");
    assert!(
        pixel[0] == pixel[1] && pixel[1] == pixel[2],
        "grey pixel: {pixel:?}"
    );
    pixel[0]
}

#[test]
fn a_one_to_one_image_is_its_own_texels() {
    // The checker at 2x2 device pixels from (10, 10): each destination pixel
    // centre maps onto a texel centre, so every pixel is a pure texel. The
    // subpixel sampling of the fixed supersample blended each texel with its
    // neighbours as [1/8, 3/4, 1/8] on each axis instead (this checker's
    // black corner texel came out at grey level 56, its blur clamped at the
    // image edge).
    let png = render_png(&checker_scene(
        r#""x":0,"y":0,"width":2,"height":2,"angle":0,"opacity":100,"scale":[1,1]"#,
    ));
    assert_eq!(png.pixel(10, 10), BLACK);
    assert_eq!(png.pixel(11, 10), WHITE);
    assert_eq!(png.pixel(10, 11), WHITE);
    assert_eq!(png.pixel(11, 11), BLACK);
    assert_eq!(png.pixel(12, 10), CLEAR);
    assert_eq!(png.pixel(10, 12), CLEAR);
}

#[test]
fn an_upscaled_image_has_the_canvas_bilinear_ramp_between_texel_centres() {
    // The checker at 8x8 device pixels (4x): a destination pixel's centre
    // `(i + 0.5) / 4` maps to source `i / 4 - 0.375`, so pixels 0 and 1 of a
    // row lie past the first texel's centre and are that texel, pixels 6 and 7
    // are the second, and pixels 2 to 5 ramp between them at 1/8, 3/8, 5/8
    // and 7/8 of the way. The top row (source v = -0.375 and -0.125, on the
    // first texel row) reads black then white. Here every texel centre and
    // every clamp boundary falls on a subpixel boundary, so the four subpixel
    // samples averaged to nearly the same values. The exact bytes below were
    // checked in two fresh Chromium 149.0.7827.55 processes:
    // the canvas truncates 255 * 1/8 and 255 * 3/8 rather than rounding them.
    let png = render_png(&checker_scene(
        r#""x":0,"y":0,"width":8,"height":8,"angle":0,"opacity":100,"scale":[1,1]"#,
    ));
    let row: Vec<u8> = (0..8).map(|x| level(png.pixel(10 + x, 10))).collect();
    assert_eq!(row, [0, 0, 31, 95, 159, 223, 255, 255]);
    // The second texel row (device rows 16 and 17) is white then black.
    let row: Vec<u8> = (0..8).map(|x| level(png.pixel(10 + x, 17))).collect();
    assert_eq!(row, [255, 255, 223, 159, 95, 31, 0, 0]);
    // The middle rows ramp vertically as well: device row 13 (source
    // v = 0.375) mixes the two texel rows 5/8 to 3/8, so the leftmost pixel
    // (source u past the left edge, the first column) is 3/8 white.
    assert_eq!(level(png.pixel(10, 13)), 95);
    assert_eq!(level(png.pixel(17, 13)), 159);
}

#[test]
fn a_texel_centre_inside_a_destination_pixel_is_sampled_once_not_averaged() {
    // The checker at 6x6 device pixels (3x) from (10, 10): pixel 1 of a row
    // has its centre `1.5 / 3 = 0.5` exactly on the first texel's centre, so
    // the canvas reads the texel itself (black in the top row); averaging four
    // subpixel samples, two of them past the centre on the ramp toward white,
    // read 11 instead. Pixels 2 and 3 map to 1/3 and 2/3, which Chromium's
    // four-bit weights truncate to 5/16 and 10/16, giving levels 79 and 159.
    // Pixel 4's centre maps to the second texel's centre in exact arithmetic,
    // but Skia's legacy sampler steps x in 32.32 fixed point from the binary32
    // inverse of the device matrix and lands just short of it, so the pixel
    // is 15/16 white (239), as the pinned Chromium 149.0.7827.55 canvas
    // paints it (probed for #198).
    let png = render_png(&checker_scene(
        r#""x":0,"y":0,"width":6,"height":6,"angle":0,"opacity":100,"scale":[1,1]"#,
    ));
    let row: Vec<u8> = (0..6).map(|x| level(png.pixel(10 + x, 10))).collect();
    assert_eq!(row, [0, 0, 79, 159, 239, 255]);
    // At a half-pixel offset (x = 0.5, 8x8) the snapped rectangle starts at
    // device column 11 and pixel 12's centre maps onto the first texel's
    // centre exactly (source u = 0): a pure texel, not the 8-level blend the
    // subpixel samples straddling the centre gave.
    let png = render_png(&checker_scene(
        r#""x":0.5,"y":0,"width":8,"height":8,"angle":0,"opacity":100,"scale":[1,1]"#,
    ));
    assert_eq!(png.pixel(10, 10), CLEAR);
    let row: Vec<u8> = (11..19).map(|x| level(png.pixel(x, 10))).collect();
    assert_eq!(row, [0, 0, 63, 127, 191, 255, 255, 255]);
}

#[test]
fn a_rotated_image_keeps_the_supersampled_path() {
    // A rotated image is not snapped and keeps its antialiased edges: the
    // rule above is the canvas's for a transform that keeps rectangles
    // rectangles only.
    let png = render_png(&checker_scene(
        r#""x":0,"y":0,"width":8,"height":8,"angle":0.4,"opacity":100,"scale":[1,1]"#,
    ));
    let blended = (0..png.height as usize)
        .flat_map(|y| (0..png.width as usize).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let alpha = png.pixel(x, y)[3];
            alpha != 0 && alpha != 255
        })
        .count();
    assert!(blended > 0, "a rotated image's edges are antialiased");
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
