//! The reference SVG export draws an image through a `<use>` of
//! `Math.round(element.width)` by `Math.round(element.height)`, the one image
//! value spec section 8.6 pins to the official integer rounding. SVG and PDF,
//! whose visual authority is that export, size the image viewport that way;
//! the crop offset, the flip about the element's center, the rotation, the
//! corner clip and the link rectangle keep the element's unrounded size. PNG
//! follows the canvas export, whose `drawImage` fills the unrounded element
//! rectangle, so a fractional edge stays a fractional edge there (#131).

use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use std::io::Cursor;

/// A one-image scene of a 2x2 solid red PNG; `element_fields` complete the
/// image element.
fn red_image_scene(element_fields: &str) -> SceneDocument {
    let png = fixture_png(2, 2, &[[255, 0, 0, 255]; 4]);
    image_scene(&png, element_fields)
}

fn image_scene(png: &[u8], element_fields: &str) -> SceneDocument {
    asset_image_scene("image/png", png, element_fields)
}

fn asset_image_scene(media_type: &str, payload: &[u8], element_fields: &str) -> SceneDocument {
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[{{
                "id":"image","type":"image","fileId":"asset","status":"saved",
                "seed":1,"updated":1,{element_fields}
            }}],
            "appState":{{}},
            "files":{{"asset":{{"id":"asset","mimeType":"{media_type}","dataURL":"data:{media_type};base64,{}","created":1}}}}
        }}"#,
        base64::engine::general_purpose::STANDARD.encode(payload)
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}

#[test]
fn an_uncropped_fractional_image_fills_the_rounded_viewport_in_svg_and_pdf_and_the_canvas_extent_in_png(
) {
    // 40.5 rounds up and 40.4 down; the rotation center stays the unrounded
    // element center, as the reference's `cx`/`cy` do.
    let scene = red_image_scene(
        r#""x":0,"y":0,"width":40.5,"height":40.4,"angle":0,"opacity":100,"scale":[1,1]"#,
    );

    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(
        svg.contains("<g transform=\"translate(10 10) rotate(0 20.25 20.2)\">"),
        "SVG rotation center is not the unrounded element center: {svg}"
    );
    assert!(
        svg.contains("\" preserveAspectRatio=\"none\" width=\"41\" height=\"40\"/></g>"),
        "SVG image is not the rounded reference viewport: {svg}"
    );

    let pdf = render_artifact(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains("0 0 41 40 re\nW\nn\n41 0 0 40 0 0 cm\n/Im0 Do"),
        "PDF image viewport is not the rounded reference size: {pdf}"
    );

    // The canvas draws the image over device [10, 50.5) x [10, 50.4) of the
    // truncated 60x60 surface: the column at 50 is half covered, the one at
    // 51 untouched.
    let (pixels, width, height) = render_png_pixels(&scene);
    assert_eq!((width, height), (60, 60));
    assert_eq!(pixel(&pixels, width, 49, 30), [255, 0, 0, 255]);
    let edge = pixel(&pixels, width, 50, 30);
    assert!(
        (64..=192).contains(&edge[3]),
        "PNG right edge is not a half-covered fractional edge: {edge:?}"
    );
    assert_eq!(pixel(&pixels, width, 51, 30), [0, 0, 0, 0]);
}

#[test]
fn a_flipped_fractional_image_mirrors_its_rounded_viewport_about_the_unrounded_center() {
    // A 2x1 green|red source flipped horizontally into 40.5x40.4. The
    // reference mirrors its `<use>` about `cx = width / 2`, so the rounded
    // viewport lands on [-0.5, 40.5] in element space, not [0, 41].
    let png = fixture_png(2, 1, &[[0, 255, 0, 255], [255, 0, 0, 255]]);
    let scene = image_scene(
        &png,
        r#""x":0,"y":0,"width":40.5,"height":40.4,"angle":0,"opacity":100,"scale":[-1,1]"#,
    );

    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(
        svg.contains(
            "<g transform=\"translate(40.5 0) scale(-1 1)\"><image href=\"data:image/png;base64,"
        ),
        "SVG flip translation is not the unrounded width: {svg}"
    );
    assert!(
        svg.contains("\" preserveAspectRatio=\"none\" width=\"41\" height=\"40\"/></g></g>"),
        "SVG image is not the rounded viewport: {svg}"
    );

    // The PDF clip is the mirrored viewport and the content matrix mirrors the
    // asset about the unrounded center: `x' = 40.5 - 41 * u`.
    let pdf = render_artifact(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains("-0.5 0 41 40 re\nW\nn\n-41 0 0 40 40.5 0 cm\n/Im0 Do"),
        "PDF does not mirror the rounded viewport about the unrounded center: {pdf}"
    );

    // The canvas mirrors the unrounded rectangle: red now sits at the left,
    // green at the right, and the right edge is still the half-covered column.
    let (pixels, width, _) = render_png_pixels(&scene);
    let left = pixel(&pixels, width, 12, 30);
    assert!(
        left[0] > 240 && left[1] < 20 && left[3] == 255,
        "PNG left is not red: {left:?}"
    );
    let right = pixel(&pixels, width, 48, 30);
    assert!(
        right[0] < 20 && right[1] > 240 && right[3] == 255,
        "PNG right is not green: {right:?}"
    );
    let edge = pixel(&pixels, width, 50, 30);
    assert!(
        (64..=192).contains(&edge[3]),
        "PNG right edge is not a half-covered fractional edge: {edge:?}"
    );
}

#[test]
fn a_cropped_flipped_fractional_image_keeps_the_reference_mask_viewport_and_unrounded_content() {
    // A 3x2 source cropped to its right 2x2 and flipped on both axes into a
    // 20.5x20.5 element. The reference draws the whole image at its unrounded
    // uncropped 30.75x20.5 under the 10.25 crop offset, through a `<use>`
    // viewport of `round(width) + normalizedCropX` mirrored with the image,
    // behind a rounded 21x21 crop mask on the unflipped group.
    let png = fixture_png(
        3,
        2,
        &[
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [0, 255, 255, 255],
            [255, 0, 255, 255],
            [255, 255, 0, 255],
        ],
    );
    let scene = image_scene(
        &png,
        concat!(
            r#""x":0,"y":0,"width":20.5,"height":20.5,"angle":0,"opacity":100,"scale":[-1,-1],"#,
            r#""crop":{"x":1,"y":0,"width":2,"height":2,"naturalWidth":3,"naturalHeight":2}"#
        ),
    );

    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(
        svg.contains(concat!(
            "<g transform=\"translate(10 10) rotate(0 10.25 10.25)\">",
            "<svg width=\"21\" height=\"21\" overflow=\"hidden\">",
            "<g transform=\"translate(20.5 20.5) scale(-1 -1)\">",
            "<svg x=\"-10.25\" y=\"0\" width=\"31.25\" height=\"21\" overflow=\"hidden\">",
            "<image href=\"data:image/png;base64,",
        )),
        "SVG lacks the reference mask, flip and viewport: {svg}"
    );
    assert!(
        svg.contains(
            "\" preserveAspectRatio=\"none\" width=\"30.75\" height=\"20.5\"/></svg></g></svg></g>"
        ),
        "SVG image is not the unrounded uncropped size: {svg}"
    );

    // PDF clips once to what the reference leaves visible: the 21x21 mask
    // meets the mirrored viewport `[-0.5, 30.75] x [-0.5, 20.5]` in
    // `[0, 21] x [0, 20.5]`; then the whole asset at its uncropped size is
    // mirrored about the center, its left edge at `width + normalizedCropX`.
    let pdf = render_artifact(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains(concat!(
            "0 0 21 20.5 re\nW\nn\n",
            "-30.75 0 0 -20.5 30.75 20.5 cm\n/Im0 Do"
        )),
        "PDF lacks the reference visible rectangle and content matrix: {pdf}"
    );

    // The canvas maps the crop into the unrounded 20.5x20.5 and flips it:
    // yellow / magenta over blue / green, as for an integer-sized element.
    let (pixels, width, height) = render_png_pixels(&scene);
    assert_eq!((width, height), (40, 40));
    assert_eq!(pixel(&pixels, width, 14, 14), [255, 255, 0, 255]);
    let magenta = pixel(&pixels, width, 25, 14);
    assert!(magenta[0] > 240 && magenta[1] < 20 && magenta[2] > 240 && magenta[3] == 255);
    let blue = pixel(&pixels, width, 14, 25);
    assert!(blue[0] < 20 && blue[1] < 20 && blue[2] > 240 && blue[3] == 255);
    let green = pixel(&pixels, width, 25, 25);
    assert!(green[0] < 20 && green[1] > 225 && green[2] < 30 && green[3] == 255);
    let edge = pixel(&pixels, width, 30, 20);
    assert!(
        (64..=192).contains(&edge[3]),
        "PNG right edge is not a half-covered fractional edge: {edge:?}"
    );
}

#[test]
fn a_nested_svg_in_pdf_inverts_its_form_within_its_own_content_rectangle() {
    // svg2pdf Forms carry SVG's Y axis inverted, so PDF draws a nested SVG's
    // form Y-inverted in place: about the content rectangle, not the
    // element's center. The two coincide only while the content is the
    // element's own rectangle, which a rounded viewport or a crop breaks.
    let nested = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect width="20" height="10" fill="#ff0000"/><rect y="10" width="20" height="10" fill="#0000ff"/></svg>"##;

    // Uncropped, 40x40.4: the content is the rounded 40x40 viewport.
    let scene = asset_image_scene(
        "image/svg+xml",
        nested,
        r#""x":0,"y":0,"width":40,"height":40.4,"angle":0,"opacity":100,"scale":[1,1]"#,
    );
    let pdf = render_artifact(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains("0 0 40 40 re\nW\nn\n40 0 0 -40 0 40 cm\n/Svg0 Do"),
        "PDF does not invert the form within the rounded viewport: {pdf}"
    );

    // Cropped to the source's red top half into 20x10: the whole 20x20 source
    // is drawn under the 10 px viewport, inverted about its own rectangle so
    // the top half is what shows.
    let crop =
        r#""crop":{"x":0,"y":0,"width":20,"height":10,"naturalWidth":20,"naturalHeight":20}"#;
    let scene = asset_image_scene(
        "image/svg+xml",
        nested,
        &format!(
            r#""x":0,"y":0,"width":20,"height":10,"angle":0,"opacity":100,"scale":[1,1],{crop}"#
        ),
    );
    let pdf = render_artifact(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains("0 0 20 10 re\nW\nn\n20 0 0 -20 0 20 cm\n/Svg0 Do"),
        "PDF does not invert the cropped form within its content rectangle: {pdf}"
    );
    let (pixels, width, _) = render_png_pixels(&scene);
    assert_eq!(pixel(&pixels, width, 20, 15), [255, 0, 0, 255]);

    // The same crop flipped vertically: the element's flip about its center
    // composes with the form's inversion into an upright form under the
    // mirrored viewport.
    let scene = asset_image_scene(
        "image/svg+xml",
        nested,
        &format!(
            r#""x":0,"y":0,"width":20,"height":10,"angle":0,"opacity":100,"scale":[1,-1],{crop}"#
        ),
    );
    let pdf = render_artifact(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains("0 0 20 10 re\nW\nn\n20 0 0 20 0 -10 cm\n/Svg0 Do"),
        "PDF does not compose the flip with the form inversion: {pdf}"
    );
    let (pixels, width, _) = render_png_pixels(&scene);
    assert_eq!(pixel(&pixels, width, 20, 15), [255, 0, 0, 255]);
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

fn render_artifact(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    let request = RenderRequest::builder(format)
        .include_background(false)
        .build()
        .expect("request is valid");
    let outcome = Renderer::new()
        .render(scene, &request)
        .expect("fractional image renders");
    assert!(
        outcome.report().is_empty(),
        "unexpected diagnostics: {:?}",
        outcome.report().diagnostics()
    );
    outcome.artifact().bytes().to_vec()
}

fn render_png_pixels(scene: &SceneDocument) -> (Vec<u8>, usize, usize) {
    let png = render_artifact(scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut pixels).expect("PNG frame decodes");
    pixels.truncate(output.buffer_size());
    (
        pixels,
        usize::try_from(output.width).expect("width fits usize"),
        usize::try_from(output.height).expect("height fits usize"),
    )
}

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    pixels[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .expect("pixel has four channels")
}
