//! The reference SVG export draws an image through a `<use>` of
//! `Math.round(element.width)` by `Math.round(element.height)`, the one image
//! value spec section 8.6 pins to the official integer rounding. SVG and PDF,
//! whose visual authority is that export, size the image viewport that way;
//! the crop offset, the flip about the element's center, the rotation, the
//! corner clip and the link rectangle keep the element's unrounded size. PNG
//! follows the canvas export, whose `drawImage` maps the image onto the
//! unrounded element rectangle and, for an axis-aligned image, paints exactly
//! the device pixels whose centres that rectangle covers with no edge
//! antialiasing (#131, #162): a fractional edge is a whole row or column or
//! none, never a blend, and the geometry stays fractional in the sampling.

use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{OutputFormat, SceneDocument};

use crate::common::image_scene::image_scene_with_element_fields;
use crate::common::raster::{decode_png, DecodedPng};
use crate::common::render::render_artifact_without_background;
use crate::common::svg::image_use;

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
    image_scene_with_element_fields(Some((media_type, payload)), element_fields)
}

#[test]
fn an_uncropped_fractional_image_fills_the_rounded_viewport_in_svg_and_pdf_and_the_canvas_extent_in_png(
) {
    // 40.5 rounds up and 40.4 down; the rotation center stays the unrounded
    // element center, as the reference's `cx`/`cy` do.
    let scene = red_image_scene(
        r#""x":0,"y":0,"width":40.5,"height":40.4,"angle":0,"opacity":100,"scale":[1,1]"#,
    );

    let svg = render_artifact_without_background(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(
        svg.contains("<g transform=\"translate(10 10) rotate(0 20.25 20.2)\">"),
        "SVG rotation center is not the unrounded element center: {svg}"
    );
    assert!(
        svg.contains(&format!("{}</g>", image_use(0, "41", "40", ""))),
        "SVG image is not the rounded reference viewport: {svg}"
    );

    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    assert!(
        pdf.contains("0 0 41 40 re\nW\nn\n41 0 0 40 0 0 cm\n/Im0 Do"),
        "PDF image viewport is not the rounded reference size: {pdf}"
    );

    // The canvas draws the image over device [10, 50.5) x [10, 50.4) of the
    // truncated 60x60 surface by pixel-centre coverage (#162): column 50,
    // whose centre 50.5 is on the right edge, is painted whole; row 50, whose
    // centre 50.5 is below the bottom edge 50.4, is not painted at all; the
    // column at 51 is untouched. No edge is a blend.
    let png = render_png(&scene);
    assert_eq!((png.width, png.height), (60, 60));
    assert_eq!(png.pixel(49, 30), [255, 0, 0, 255]);
    assert_eq!(
        png.pixel(50, 30),
        [255, 0, 0, 255],
        "PNG right edge column is painted whole, its centre lying on the edge"
    );
    assert_eq!(png.pixel(51, 30), [0, 0, 0, 0]);
    assert_eq!(png.pixel(30, 49), [255, 0, 0, 255]);
    assert_eq!(
        png.pixel(30, 50),
        [0, 0, 0, 0],
        "PNG bottom edge row is left empty, its centre lying below the edge"
    );
}

#[test]
pub(super) fn a_flipped_fractional_image_mirrors_its_rounded_viewport_about_the_unrounded_center() {
    // A 2x1 green|red source flipped horizontally into 40.5x40.4. The
    // reference mirrors its `<use>` about `cx = width / 2`, so the rounded
    // viewport lands on [-0.5, 40.5] in element space, not [0, 41].
    let png = fixture_png(2, 1, &[[0, 255, 0, 255], [255, 0, 0, 255]]);
    let scene = image_scene(
        &png,
        r#""x":0,"y":0,"width":40.5,"height":40.4,"angle":0,"opacity":100,"scale":[-1,1]"#,
    );

    let svg = render_artifact_without_background(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(
        svg.contains(&format!(
            "<g transform=\"translate(40.5 0) scale(-1 1)\">{}</g></g>",
            image_use(0, "41", "40", "")
        )),
        "SVG flip translation is not the unrounded width around the rounded viewport: {svg}"
    );

    // The PDF clip is the mirrored viewport and the content matrix mirrors the
    // asset about the unrounded center: `x' = 40.5 - 41 * u`.
    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    assert!(
        pdf.contains("-0.5 0 41 40 re\nW\nn\n-41 0 0 40 40.5 0 cm\n/Im0 Do"),
        "PDF does not mirror the rounded viewport about the unrounded center: {pdf}"
    );

    // The canvas mirrors the unrounded rectangle: red now sits at the left,
    // green at the right, and the right edge column at 50, whose centre lies
    // on the edge 50.5, is painted whole with the clamped edge pixel (#162).
    let png = render_png(&scene);
    let left = png.pixel(12, 30);
    assert!(
        left[0] > 240 && left[1] < 20 && left[3] == 255,
        "PNG left is not red: {left:?}"
    );
    let right = png.pixel(48, 30);
    assert!(
        right[0] < 20 && right[1] > 240 && right[3] == 255,
        "PNG right is not green: {right:?}"
    );
    let edge = png.pixel(50, 30);
    assert!(
        edge[0] < 20 && edge[1] > 240 && edge[3] == 255,
        "PNG right edge column is not the whole green edge pixel: {edge:?}"
    );
    assert_eq!(png.pixel(51, 30), [0, 0, 0, 0]);
}

#[test]
pub(super) fn a_cropped_flipped_fractional_image_keeps_the_reference_mask_viewport_and_unrounded_content(
) {
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

    let svg = render_artifact_without_background(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(
        svg.contains(&format!(
            concat!(
                "<g transform=\"translate(10 10) rotate(0 10.25 10.25)\">",
                "<svg width=\"21\" height=\"21\" overflow=\"hidden\">",
                "<g transform=\"translate(20.5 20.5) scale(-1 -1)\">",
                "<svg x=\"-10.25\" y=\"0\" width=\"31.25\" height=\"21\" overflow=\"hidden\">",
                "{}</svg></g></svg></g>",
            ),
            image_use(0, "30.75", "20.5", "")
        )),
        "SVG lacks the reference mask, flip and viewport around the unrounded uncropped image: {svg}"
    );

    // PDF clips once to what the reference leaves visible: the 21x21 mask
    // meets the mirrored viewport `[-0.5, 30.75] x [-0.5, 20.5]` in
    // `[0, 21] x [0, 20.5]`; then the whole asset at its uncropped size is
    // mirrored about the center, its left edge at `width + normalizedCropX`.
    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    assert!(
        pdf.contains(concat!(
            "0 0 21 20.5 re\nW\nn\n",
            "-30.75 0 0 -20.5 30.75 20.5 cm\n/Im0 Do"
        )),
        "PDF lacks the reference visible rectangle and content matrix: {pdf}"
    );

    // The canvas maps the crop into the unrounded 20.5x20.5 and flips it:
    // yellow / magenta over blue / green, as for an integer-sized element.
    let png = render_png(&scene);
    assert_eq!((png.width, png.height), (40, 40));
    assert_eq!(png.pixel(14, 14), [255, 255, 0, 255]);
    let magenta = png.pixel(25, 14);
    assert!(magenta[0] > 240 && magenta[1] < 20 && magenta[2] > 240 && magenta[3] == 255);
    let blue = png.pixel(14, 25);
    assert!(blue[0] < 20 && blue[1] < 20 && blue[2] > 240 && blue[3] == 255);
    let green = png.pixel(25, 25);
    assert!(green[0] < 20 && green[1] > 225 && green[2] < 30 && green[3] == 255);
    // The element spans device [10, 30.5) on both axes: column 30 and row 30,
    // whose centres lie on the edges, are painted whole and opaque (#162).
    // Their colour blends the crop's edge with the column the crop leaves
    // out, as the canvas's bilinear sampling of the whole image does at the
    // crop boundary (the official evidence of `focused.image-codecs.v1` shows
    // that bleed); coverage, not colour, is what the rule fixes.
    let edge = png.pixel(30, 14);
    assert_eq!(
        edge[3], 255,
        "PNG right edge column is not opaque: {edge:?}"
    );
    let corner = png.pixel(30, 30);
    assert_eq!(
        corner[3], 255,
        "PNG bottom-right edge pixel is not opaque: {corner:?}"
    );
    assert_eq!(png.pixel(31, 20), [0, 0, 0, 0]);
    assert_eq!(png.pixel(20, 31), [0, 0, 0, 0]);
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
    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
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
    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    assert!(
        pdf.contains("0 0 20 10 re\nW\nn\n20 0 0 -20 0 20 cm\n/Svg0 Do"),
        "PDF does not invert the cropped form within its content rectangle: {pdf}"
    );
    let png = render_png(&scene);
    assert_eq!(png.pixel(20, 15), [255, 0, 0, 255]);

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
    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    assert!(
        pdf.contains("0 0 20 10 re\nW\nn\n20 0 0 20 0 -10 cm\n/Svg0 Do"),
        "PDF does not compose the flip with the form inversion: {pdf}"
    );
    let png = render_png(&scene);
    assert_eq!(png.pixel(20, 15), [255, 0, 0, 255]);
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

fn render_png(scene: &SceneDocument) -> DecodedPng {
    decode_png(&render_artifact_without_background(
        scene,
        OutputFormat::Png,
    ))
}
