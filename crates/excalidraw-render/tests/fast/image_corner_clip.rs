//! An image element with a `roundness` record is clipped to its rectangle with
//! `getCornerRadius(min(width, height))` corners, as the reference canvas clips
//! it with `roundRect` before `drawImage` (#129); the reference SVG declares the
//! same `<clipPath rx ry>` but browsers do not apply its namespaced `clip-path`.
//! The clip lives in the element's own space, so crop, negative `scale` flips
//! and rotation leave the corners at the element's corners; the image/error
//! fallback is never clipped, as the reference canvas draws its placeholder
//! unclipped.

use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{OutputFormat, SceneDocument};

use crate::common::image_scene::image_scene_with_element_fields;
use crate::common::raster::{decode_png, DecodedPng};
use crate::common::render::{render_artifact_without_background, render_without_background};

/// A one-image scene: `element_fields` complete the image element, and `png`
/// is its file's payload, or `None` for a scene whose file is missing.
fn image_scene(png: Option<&[u8]>, element_fields: &str) -> SceneDocument {
    image_scene_with_element_fields(png.map(|png| ("image/png", png)), element_fields)
}

/// A 40x40 image of a 2x2 solid red PNG; `extra_fields` add to the element.
/// With the editor's default adaptive roundness (`{type: 3}`, 32 px),
/// `getCornerRadius(40)` is `min(32, 10)` = 10.
fn red_square_scene(extra_fields: &str) -> SceneDocument {
    let png = fixture_png(2, 2, &[[255, 0, 0, 255]; 4]);
    image_scene(
        Some(&png),
        &format!(
            r#""x":0,"y":0,"width":40,"height":40,"angle":0,"opacity":100,"scale":[1,1]{extra_fields}"#
        ),
    )
}

#[test]
fn a_rounded_image_is_clipped_to_the_official_corner_radius_in_every_format() {
    let scene = red_square_scene(r#","roundness":{"type":3}"#);

    let svg = render_artifact_without_background(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    // The corner clip precedes the shared image definition in `<defs>`
    // (spec 8.4), and the clip sits on the element's group.
    assert!(
        svg.contains(concat!(
            "<defs><clipPath id=\"image-clip-0\">",
            "<rect width=\"40\" height=\"40\" rx=\"10\" ry=\"10\"/>",
            "</clipPath><symbol id=\"image-0\"><image href=\"data:image/png;base64,",
        )) && svg.contains(concat!(
            "</symbol></defs>",
            "<g transform=\"translate(10 10) rotate(0 20 20)\" clip-path=\"url(#image-clip-0)\">",
        )),
        "SVG lacks the image corner clip before the shared image: {svg}"
    );

    let png = render_png(&scene);
    assert_eq!((png.width, png.height), (60, 60));
    // The image covers device [10, 50). Its corner pixels lie outside the
    // 10 px arcs; its edge midpoints and interior lie inside them.
    for (x, y) in [(10, 10), (49, 10), (10, 49), (49, 49)] {
        assert_eq!(png.pixel(x, y), [0, 0, 0, 0], "corner ({x}, {y})");
    }
    for (x, y) in [(14, 14), (10, 30), (30, 10), (30, 30), (49, 30), (30, 49)] {
        assert_eq!(png.pixel(x, y), [255, 0, 0, 255], "inside ({x}, {y})");
    }

    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    // The clip is the rounded rectangle at full precision, replacing the
    // rectangular clip an unrounded image carries. Krilla writes it in
    // top-down page points (ADR-0021): element (10, 0) at the 10 px padding is
    // (15, 37.5) on the 45 pt page, and the image fills the 40 x 40 element.
    assert!(
        pdf.contains("15 37.5 m\n30 37.5 l\n"),
        "PDF lacks the rounded clip path: {pdf}"
    );
    assert!(
        pdf.contains(" c\nW\nn\n/Artifact BMC\nq\n30 0 0 30 7.5 7.5 cm\n/x0 Do"),
        "PDF clip is not a path clip before the image: {pdf}"
    );
    assert!(!pdf.contains("7.5 37.5 m\n37.5 37.5 l\n37.5 7.5 l\n7.5 7.5 l\nh\nW"));
}

#[test]
fn an_unrounded_image_keeps_its_rectangular_bounds_in_every_format() {
    let scene = red_square_scene("");

    let svg = render_artifact_without_background(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(!svg.contains("clipPath"));
    assert!(svg.contains("<g transform=\"translate(10 10) rotate(0 20 20)\">"));

    let png = render_png(&scene);
    for (x, y) in [(10, 10), (49, 10), (10, 49), (49, 49), (30, 30)] {
        assert_eq!(png.pixel(x, y), [255, 0, 0, 255], "({x}, {y})");
    }

    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    // The element's own 40 x 40 bounds clip it, in page points.
    assert!(pdf.contains(concat!(
        "7.5 37.5 m\n37.5 37.5 l\n37.5 7.5 l\n7.5 7.5 l\nh\nW\nn\n",
        "/Artifact BMC\nq\n30 0 0 30 7.5 7.5 cm\n/x0 Do"
    )));
}

#[test]
fn the_corner_clip_stays_in_element_space_when_the_image_is_cropped_and_flipped() {
    // A 3x2 source cropped to its right 2x2 and flipped on both axes into a
    // 20x20 element, as in the static PNG crop test, with a fixed 8 px radius
    // that the proportional cutoff lowers to min(8, 20 * 0.25) = 5.
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
        Some(&png),
        concat!(
            r#""x":0,"y":0,"width":20,"height":20,"angle":0,"opacity":100,"scale":[-1,-1],"#,
            r#""crop":{"x":1,"y":0,"width":2,"height":2,"naturalWidth":3,"naturalHeight":2},"#,
            r#""roundness":{"type":3,"value":8}"#
        ),
    );

    let svg = render_artifact_without_background(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    // The clip rectangle is the element's 20x20, not the crop's, and it sits
    // on the translated group outside the crop mask, the flip and the viewport.
    assert!(svg.contains(
        "<clipPath id=\"image-clip-0\"><rect width=\"20\" height=\"20\" rx=\"5\" ry=\"5\"/></clipPath>"
    ));
    assert!(svg.contains(concat!(
        "<g transform=\"translate(10 10) rotate(0 10 10)\" clip-path=\"url(#image-clip-0)\">",
        "<svg width=\"20\" height=\"20\" overflow=\"hidden\">",
        "<g transform=\"translate(20 20) scale(-1 -1)\">",
        "<svg x=\"-10\" y=\"0\" width=\"30\" height=\"20\" overflow=\"hidden\">",
    )));

    let png = render_png(&scene);
    assert_eq!((png.width, png.height), (40, 40));
    for (x, y) in [(10, 10), (29, 10), (10, 29), (29, 29)] {
        assert_eq!(png.pixel(x, y), [0, 0, 0, 0], "corner ({x}, {y})");
    }
    // The flipped crop still lands yellow / magenta / blue / green.
    let yellow = png.pixel(14, 14);
    let magenta = png.pixel(24, 14);
    let blue = png.pixel(14, 24);
    let green = png.pixel(24, 24);
    assert_eq!(yellow, [255, 255, 0, 255]);
    assert!(magenta[0] > 240 && magenta[1] < 20 && magenta[2] > 240 && magenta[3] == 255);
    assert!(blue[0] < 20 && blue[1] < 20 && blue[2] > 240 && blue[3] == 255);
    assert!(green[0] < 20 && green[1] > 225 && green[2] < 30 && green[3] == 255);

    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    // Element (5, 0) at the 10 px padding is (11.25, 22.5) on the 30 pt page.
    assert!(
        pdf.contains("11.25 22.5 m\n18.75 22.5 l\n"),
        "PDF lacks the rounded clip path: {pdf}"
    );
    // The corner clip comes first, then the crop's visible rectangle (its
    // mask meeting the mirrored viewport), then the whole 3x2 asset at its
    // uncropped 30x20 mirrored about the center (element matrix
    // -30 0 0 20 30 0, in page points).
    assert!(pdf.contains(concat!(
        " c\nW\nn\nq\n",
        "7.5 22.5 m\n22.5 22.5 l\n22.5 7.5 l\n7.5 7.5 l\nh\nW\nn\n",
        "/Artifact BMC\nq\n-22.5 0 0 -15 30 22.5 cm\n/x0 Do"
    )));
}

#[test]
fn the_corner_clip_rotates_with_the_image() {
    // A 40x20 image turned a quarter turn about its center occupies device
    // [10, 30) x [10, 50); its corners are still the clipped ones and the
    // midpoints of its now-vertical long edges are painted.
    let png = fixture_png(2, 2, &[[255, 0, 0, 255]; 4]);
    let scene = image_scene(
        Some(&png),
        concat!(
            r#""x":0,"y":0,"width":40,"height":20,"angle":1.5707963267948966,"#,
            r#""opacity":100,"scale":[1,1],"roundness":{"type":3}"#
        ),
    );

    let svg = render_artifact_without_background(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    let clip_reference = svg
        .find("clip-path=\"url(#image-clip-0)\"")
        .expect("SVG references the image clip");
    let group_start = svg[..clip_reference]
        .rfind("<g ")
        .expect("the clip reference sits on a group");
    let group = &svg[group_start..clip_reference];
    assert!(
        group.contains("rotate(90 20 10)"),
        "clip is not on the rotated group: {group}"
    );
    assert!(svg.contains("<rect width=\"40\" height=\"20\" rx=\"5\" ry=\"5\"/>"));

    let png = render_png(&scene);
    assert_eq!((png.width, png.height), (40, 60));
    for (x, y) in [(10, 10), (29, 10), (10, 49), (29, 49)] {
        assert_eq!(png.pixel(x, y), [0, 0, 0, 0], "corner ({x}, {y})");
    }
    for (x, y) in [(20, 10), (20, 49), (10, 30), (29, 30), (20, 30)] {
        assert_eq!(png.pixel(x, y), [255, 0, 0, 255], "inside ({x}, {y})");
    }

    let pdf = render_artifact_without_background(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(&pdf);
    // Element (5, 0) and (35, 0) turn a quarter about the center (20, 10)
    // and move by the (0, 20) the rotated bounds need: page (22.5, 33.75)
    // and (22.5, 11.25) on the 45 pt page.
    assert!(
        pdf.contains("22.5 33.75 m\n22.5 11.25 l\n"),
        "PDF lacks the rounded clip path: {pdf}"
    );
    assert!(!pdf.contains(" l\nh\nW\nn\n"));
}

#[test]
fn a_missing_image_fallback_is_never_corner_clipped() {
    let scene = image_scene(
        None,
        r#""x":0,"y":0,"width":40,"height":40,"angle":0,"opacity":100,"scale":[1,1],"roundness":{"type":3}"#,
    );

    let png = render_png(&scene);
    // The reference canvas clips only a drawn image; its placeholder fills the
    // whole element rectangle, corners included.
    for (x, y) in [(10, 10), (49, 10), (10, 49), (49, 49)] {
        assert_eq!(png.pixel(x, y)[3], 255, "corner ({x}, {y})");
    }
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

/// The transparent PNG raster, diagnosed or not: the missing-image fallback
/// is judged here too.
fn render_png(scene: &SceneDocument) -> DecodedPng {
    decode_png(
        render_without_background(scene, OutputFormat::Png)
            .artifact()
            .bytes(),
    )
}
