//! Dark mode reaches the frame chrome and the images the way the reference's
//! dark-mode export filters reach them (#130). The reference's SVG carries
//! `THEME_FILTER` (`invert(93%) hue-rotate(180deg)`) on its root and
//! `IMAGE_INVERT_FILTER` (`invert(100%) hue-rotate(180deg) saturate(1.25)`) on
//! each raster image's `<use>`; its canvas export sets `THEME_FILTER` on the
//! context and resets the filter to `none` for raster images. So a frame
//! outline is `#bbb` through the theme filter in every format, the canvas
//! recolors a magic frame's outline by theme before that filter, a nested-SVG
//! image inverts with the theme filter alone, a raster image keeps its pixels
//! in PNG but compresses its contrast in SVG and, following the SVG authority,
//! in PDF, and the PNG image placeholder passes through the theme filter.

use base64::Engine as _;
use excalidraw_render::{ColorMode, FrameOptions, OutputFormat, RenderRequest, SceneDocument};

use crate::common::pdf_inspect::{find_bytes, inflate_streams, pdf_text};
use crate::common::raster::{decode_png, DecodedPng};
use crate::common::render::{artifact_without_diagnostics, render_with_request};

/// An ordinary 100x60 frame at the origin and a 60x40 magic frame to its
/// right, both unnamed; the request turns frame names off so the outlines sit
/// exactly `padding` inside the canvas.
const FRAME_SCENE: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"plain","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
        {"id":"magic","type":"magicframe","x":120,"y":0,"width":60,"height":40,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

/// `#bbbbbb`, `FRAME_STYLE.strokeColor`, through the theme filter.
const DARK_FRAME_OUTLINE: [u8; 3] = [0x4c, 0x4c, 0x4c];
/// `#1d8264`, the canvas's dark-theme magic-frame outline, through the theme
/// filter.
const DARK_MAGIC_FRAME_CANVAS_OUTLINE: [u8; 3] = [0x4f, 0xa6, 0x8c];
/// The default white scene background through the theme filter.
const DARK_BACKGROUND: [u8; 4] = [0x11, 0x11, 0x11, 255];

#[test]
fn dark_mode_frame_outlines_pass_through_the_theme_filter_in_every_format() {
    let scene = SceneDocument::parse(FRAME_SCENE).expect("frame scene is admitted");

    let svg = render(&scene, OutputFormat::Svg, ColorMode::Dark);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    // The reference SVG paints every frame kind with `FRAME_STYLE.strokeColor`
    // under the root filter, magic frames included.
    for id in ["plain", "magic"] {
        let outline = frame_outline(svg, id);
        assert!(
            outline.contains("stroke=\"#4c4c4c\""),
            "{id} outline is not the filtered #bbb: {outline}"
        );
    }
    assert!(!svg.contains("#bbbbbb"), "light-mode frame stroke survives");

    let png = render_png(&scene, ColorMode::Dark);
    assert_eq!((png.width, png.height), (200, 80));
    assert_eq!(png.pixel(2, 2), DARK_BACKGROUND);
    // The 2 px outline is centered on the frame edge, so the pixel just inside
    // the left edge, halfway down, is solid stroke over the background.
    assert_eq!(
        png.pixel(10, 40),
        rgba(DARK_FRAME_OUTLINE),
        "ordinary frame outline"
    );
    // The canvas recolors a magic frame's outline `#1d8264` in the dark theme
    // and the theme filter then lowers that, not the light `#7affd7`.
    assert_eq!(
        png.pixel(130, 30),
        rgba(DARK_MAGIC_FRAME_CANVAS_OUTLINE),
        "magic frame outline"
    );

    let pdf = render(&scene, OutputFormat::Pdf, ColorMode::Dark);
    let pdf = pdf_text(&pdf);
    let dark_outline_strokes = pdf
        .matches("0.2980392156862745 0.2980392156862745 0.2980392156862745 SCN\n")
        .count();
    assert_eq!(
        dark_outline_strokes, 2,
        "both PDF frame outlines stroke the filtered #bbb: {pdf}"
    );
    assert!(
        !pdf.contains("0.7333333333333333 SCN"),
        "light-mode PDF frame stroke survives"
    );
}

#[test]
fn light_mode_frame_outlines_keep_the_reference_light_constants() {
    let scene = SceneDocument::parse(FRAME_SCENE).expect("frame scene is admitted");

    let svg = render(&scene, OutputFormat::Svg, ColorMode::Light);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    for id in ["plain", "magic"] {
        assert!(frame_outline(svg, id).contains("stroke=\"#bbbbbb\""));
    }

    let png = render_png(&scene, ColorMode::Light);
    assert_eq!(png.pixel(10, 40), [0xbb, 0xbb, 0xbb, 255]);
    assert_eq!(png.pixel(130, 30), [0x7a, 0xff, 0xd7, 255]);

    let pdf = render(&scene, OutputFormat::Pdf, ColorMode::Light);
    let pdf = pdf_text(&pdf);
    assert_eq!(
        pdf.matches("0.7333333333333333 0.7333333333333333 0.7333333333333333 SCN\n")
            .count(),
        2
    );
}

/// The reference SVG puts `IMAGE_INVERT_FILTER` on a raster image's `<use>`
/// and only the root `THEME_FILTER` on a nested-SVG image's; both filters are
/// lowered into the artifact as `<defs>` filters so the payload bytes stay the
/// validated octets (spec section 8.2). The literals are worked from the Filter
/// Effects equivalents of the official filter functions, in their order:
/// `invert(a)` is `feComponentTransfer` with the table `a, 1 - a` on each
/// channel, `hue-rotate(180deg)` is `feColorMatrix hueRotate 180`, and
/// `saturate(1.25)` is the explicit `feColorMatrix type="matrix"` of the
/// Filter Effects saturate matrix at `s = 1.25` (`0.213 + 0.787s`,
/// `0.715 - 0.715s`, `0.072 - 0.072s`, ...), never `type="saturate"`, whose
/// SVG 1.1 domain is `[0, 1]` and which usvg/resvg clamp to the identity
/// (#161); all in sRGB like the functions. They pin the serialization the spec
/// (section 8.4) promises; the viewer tests below check what it paints.
const RASTER_IMAGE_FILTER: &str = concat!(
    "<filter id=\"dark-mode-raster-image\" color-interpolation-filters=\"sRGB\">",
    "<feComponentTransfer>",
    "<feFuncR type=\"table\" tableValues=\"1 0\"/>",
    "<feFuncG type=\"table\" tableValues=\"1 0\"/>",
    "<feFuncB type=\"table\" tableValues=\"1 0\"/>",
    "</feComponentTransfer>",
    "<feColorMatrix type=\"hueRotate\" values=\"180\"/>",
    "<feColorMatrix type=\"matrix\" values=\"",
    "1.19675 -0.17875 -0.018 0 0 ",
    "-0.05325 1.07125 -0.018 0 0 ",
    "-0.05325 -0.17875 1.232 0 0 ",
    "0 0 0 1 0\"/>",
    "<feComponentTransfer>",
    "<feFuncR type=\"table\" tableValues=\"0.93 0.07\"/>",
    "<feFuncG type=\"table\" tableValues=\"0.93 0.07\"/>",
    "<feFuncB type=\"table\" tableValues=\"0.93 0.07\"/>",
    "</feComponentTransfer>",
    "<feColorMatrix type=\"hueRotate\" values=\"180\"/>",
    "</filter>",
);
const NESTED_SVG_IMAGE_FILTER: &str = concat!(
    "<filter id=\"dark-mode-nested-svg-image\" color-interpolation-filters=\"sRGB\">",
    "<feComponentTransfer>",
    "<feFuncR type=\"table\" tableValues=\"0.93 0.07\"/>",
    "<feFuncG type=\"table\" tableValues=\"0.93 0.07\"/>",
    "<feFuncB type=\"table\" tableValues=\"0.93 0.07\"/>",
    "</feComponentTransfer>",
    "<feColorMatrix type=\"hueRotate\" values=\"180\"/>",
    "</filter>",
);

#[test]
fn dark_mode_svg_filters_raster_and_nested_svg_images_like_the_reference_export() {
    // The raster image is rounded, so its corner clip shows where the filters
    // sit in <defs>: after the clips, raster before nested-SVG.
    let scene = image_scene(&[
        ImageFixture {
            media_type: "image/png",
            payload: &white_png(),
            geometry: r#""x":0,"y":0,"width":20,"height":20,"roundness":{"type":3}"#,
        },
        nested_svg_fixture(r#""x":40,"y":0,"width":20,"height":20"#),
    ]);

    let dark = render(&scene, OutputFormat::Svg, ColorMode::Dark);
    let dark = std::str::from_utf8(&dark).expect("SVG is UTF-8");
    let defs = defs(dark);
    // <defs> order (spec 8.4): the image clip, the shared image definitions
    // in first-use order, then the filters, raster before nested-SVG.
    let symbols_end = defs
        .rfind("</symbol>")
        .expect("defs define the shared images")
        + "</symbol>".len();
    assert!(
        defs[..symbols_end].contains("</clipPath><symbol id=\"image-0\">")
            && defs[..symbols_end].contains("</symbol><symbol id=\"image-1\">"),
        "the shared images follow the image clip in <defs>: {defs}"
    );
    assert_eq!(
        &defs[symbols_end..],
        format!("{RASTER_IMAGE_FILTER}{NESTED_SVG_IMAGE_FILTER}"),
        "the filters follow the shared images in <defs>: {defs}"
    );
    // Each element's `<use>` carries the filter, where the reference puts
    // `IMAGE_INVERT_FILTER`; the shared `<image>` inside the symbol carries
    // none.
    assert!(
        !defs.contains(" filter="),
        "a shared image carries a filter"
    );
    let raster_use = use_element(dark, 0);
    assert!(
        raster_use.ends_with("filter=\"url(#dark-mode-raster-image)\"/>"),
        "raster image: {raster_use}"
    );
    let nested_use = use_element(dark, 1);
    assert!(
        nested_use.ends_with("filter=\"url(#dark-mode-nested-svg-image)\"/>"),
        "nested-SVG image: {nested_use}"
    );

    let light = render(&scene, OutputFormat::Svg, ColorMode::Light);
    let light = std::str::from_utf8(&light).expect("SVG is UTF-8");
    assert!(
        !light.contains("<filter"),
        "light mode defines a filter: {light}"
    );
    assert!(
        !light.contains(" filter="),
        "light mode references a filter"
    );
    for media_type in ["image/png", "image/svg+xml"] {
        assert_eq!(
            data_url(dark, media_type),
            data_url(light, media_type),
            "{media_type} payload octets change with the mode"
        );
    }
}

#[test]
fn a_viewer_applies_the_dark_mode_svg_image_filters_to_the_reference_values() {
    let scene = image_scene(&[
        white_png_fixture(r#""x":0,"y":0,"width":20,"height":20"#),
        nested_svg_fixture(r#""x":40,"y":0,"width":20,"height":20"#),
    ]);
    let svg = render(&scene, OutputFormat::Svg, ColorMode::Dark);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .expect("the dark-mode SVG parses, filters included");
    let mut pixmap = tiny_skia::Pixmap::new(80, 40).expect("pixmap allocates");
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let sample = |x: u32, y: u32| {
        let pixel = pixmap.pixel(x, y).expect("pixel is inside").demultiply();
        [pixel.red(), pixel.green(), pixel.blue()]
    };

    // White through the image filter and the root filter is #ededed; the
    // nested SVG's #1e1e1e through the root filter alone is #d3d3d3, the same
    // as a scene stroke of that color. Viewers quantize each primitive, so
    // allow one level per channel.
    assert_rgb_near(sample(20, 20), [0xed, 0xed, 0xed]);
    assert_rgb_near(sample(60, 20), [0xd3, 0xd3, 0xd3]);
    assert_eq!(sample(2, 2), [0x11, 0x11, 0x11], "background");
}

#[test]
fn a_viewer_applies_the_saturate_step_of_the_dark_mode_raster_image_filter() {
    // A gray is a fixed point of `saturate`, so the white probe above cannot
    // see the step; a saturated blue can. `#1971c2` through the reference
    // chain is what the renderer's own color math lowers into PDF
    // (`color::dark_mode_raster_image_pixel`, `[25, 120, 192]`), which is also
    // what Chromium paints for the official SVG. Spelled as `type="saturate"
    // values="1.25"`, resvg clamped the amount to 1 and painted
    // `[38, 115, 185]` instead (#161); the explicit matrix evaluates
    // literally.
    let blue = fixture_png(2, 2, &[[0x19, 0x71, 0xc2, 255]; 4]);
    let scene = image_scene(&[ImageFixture {
        media_type: "image/png",
        payload: &blue,
        geometry: r#""x":0,"y":0,"width":20,"height":20"#,
    }]);
    let svg = render(&scene, OutputFormat::Svg, ColorMode::Dark);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(
        svg.contains(
            "<feColorMatrix type=\"matrix\" values=\"1.19675 -0.17875 -0.018 0 0 -0.05325 \
             1.07125 -0.018 0 0 -0.05325 -0.17875 1.232 0 0 0 0 0 1 0\"/>"
        ),
        "{svg}"
    );
    assert!(!svg.contains("type=\"saturate\""), "{svg}");
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .expect("the dark-mode SVG parses, filters included");
    let mut pixmap = tiny_skia::Pixmap::new(40, 40).expect("pixmap allocates");
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let pixel = pixmap.pixel(20, 20).expect("pixel is inside").demultiply();
    let sample = [pixel.red(), pixel.green(), pixel.blue()];
    for (channel, expected) in sample.into_iter().zip([25, 120, 192]) {
        assert!(
            channel.abs_diff(expected) <= 2,
            "{sample:?} differs from the reference chain's [25, 120, 192] by more than \
             the viewer's per-primitive quantization"
        );
    }
    assert!(
        sample[2] > 188,
        "{sample:?}: the saturate step was not applied (an identity saturate paints ~185)"
    );
}

#[test]
fn dark_mode_svg_defines_only_the_image_filters_it_references() {
    let scene = image_scene(&[nested_svg_fixture(r#""x":0,"y":0,"width":20,"height":20"#)]);
    let svg = render(&scene, OutputFormat::Svg, ColorMode::Dark);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(defs(svg).contains(NESTED_SVG_IMAGE_FILTER));
    assert!(!svg.contains("dark-mode-raster-image"), "{svg}");
}

#[test]
fn dark_mode_png_keeps_raster_pixels_and_inverts_nested_svg_images_like_the_canvas() {
    let scene = image_scene(&[
        white_png_fixture(r#""x":0,"y":0,"width":20,"height":20"#),
        nested_svg_fixture(r#""x":40,"y":0,"width":20,"height":20"#),
    ]);

    let png = render_png(&scene, ColorMode::Dark);
    assert_eq!((png.width, png.height), (80, 40));
    assert_eq!(png.pixel(2, 2), DARK_BACKGROUND);
    // The reference canvas export resets its filter to `none` for a raster
    // image (`shouldResetImageFilter`), so the white pixels stay white.
    assert_eq!(png.pixel(20, 20), [255, 255, 255, 255]);
    // A nested-SVG image keeps the context's theme filter: #1e1e1e comes out
    // #d3d3d3, exactly as a scene stroke of that color does.
    assert_eq!(png.pixel(60, 20), [0xd3, 0xd3, 0xd3, 255]);

    let light = render_png(&scene, ColorMode::Light);
    assert_eq!(light.pixel(20, 20), [255, 255, 255, 255]);
    assert_eq!(light.pixel(60, 20), [0x1e, 0x1e, 0x1e, 255]);
}

#[test]
fn dark_mode_png_image_placeholder_passes_through_the_theme_filter() {
    // A 300x300 element whose file is missing draws the official placeholder:
    // the `#E7E7E7` rectangle and the 100 px `#888` Font Awesome image glyph
    // centered in it, both under the canvas's theme filter. The glyph's top
    // band (viewBox y 64..120) is solid; its circle (center 112,176; r 56) is
    // a hole showing the rectangle.
    let json = br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"image","type":"image","fileId":"missing","status":"saved","x":0,"y":0,"width":300,"height":300,"angle":0,"opacity":100,"scale":[1,1],"seed":1,"updated":1}],
        "appState":{},"files":{}
    }"##;
    let scene = SceneDocument::parse(json).expect("missing-image scene is admitted");
    let band = (160, 128);
    let hole = (132, 144);

    let png = render_png_with_diagnostics(&scene, ColorMode::Dark);
    assert_eq!(png.pixel(band.0, band.1), [0x78, 0x78, 0x78, 255]);
    assert_eq!(png.pixel(hole.0, hole.1), [0x26, 0x26, 0x26, 255]);

    let png = render_png_with_diagnostics(&scene, ColorMode::Light);
    assert_eq!(png.pixel(band.0, band.1), [0x88, 0x88, 0x88, 255]);
    assert_eq!(png.pixel(hole.0, hole.1), [0xe7, 0xe7, 0xe7, 255]);
}

#[test]
fn dark_mode_pdf_lowers_the_reference_svg_image_filters_into_raster_pixels() {
    // White, black, red and a fully transparent pixel.
    let png = fixture_png(
        2,
        2,
        &[
            [255, 255, 255, 255],
            [0, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 0, 0, 0],
        ],
    );
    let scene = image_scene(&[ImageFixture {
        media_type: "image/png",
        payload: &png,
        geometry: r#""x":0,"y":0,"width":20,"height":20"#,
    }]);

    let pdf = render(&scene, OutputFormat::Pdf, ColorMode::Dark);
    let (rgb, alpha) = image_xobject_streams(&pdf);
    // Official SVG is PDF's visual authority: each opaque pixel is the image
    // filter then the root filter; an invisible pixel keeps its canonical zero
    // RGB and the soft mask is untouched.
    assert_eq!(
        rgb,
        [
            [0xed, 0xed, 0xed],
            [0x11, 0x11, 0x11],
            [0xac, 0x4a, 0x4a],
            [0, 0, 0]
        ]
        .concat()
    );
    assert_eq!(alpha, [255, 255, 255, 0]);

    let pdf = render(&scene, OutputFormat::Pdf, ColorMode::Light);
    let (rgb, alpha) = image_xobject_streams(&pdf);
    assert_eq!(
        rgb,
        [[255, 255, 255], [0, 0, 0], [255, 0, 0], [0, 0, 0]].concat()
    );
    assert_eq!(alpha, [255, 255, 255, 0]);
}

/// The revision-1 limitation spec section 8.2 records for #154: PDF embeds a
/// nested-SVG image's vector content through svg2pdf in its light-mode colors.
/// This pins the exact spelling so the decision, once made, fails it.
#[test]
fn dark_mode_pdf_still_embeds_nested_svg_vector_content_in_light_colors_until_154() {
    let scene = image_scene(&[nested_svg_fixture(r#""x":0,"y":0,"width":20,"height":20"#)]);
    let pdf = render(&scene, OutputFormat::Pdf, ColorMode::Dark);
    let pdf = pdf_text(&pdf);
    // svg2pdf spells the nested rectangle's #1e1e1e fill as 30/255 in f32; the
    // lowered #d3d3d3 would be 211/255. The page background is lowered.
    assert!(
        pdf.contains("0.11764706 0.11764706 0.11764706 scn\n"),
        "{pdf}"
    );
    assert!(!pdf.contains("0.827451 0.827451 0.827451 scn"), "{pdf}");
    assert!(pdf.contains("0.06666666666666667 0.06666666666666667 0.06666666666666667 scn\n"));
}

/// A 1x1 nested SVG filled with the text color `#1e1e1e`.
const NESTED_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" viewBox="0 0 1 1"><rect width="1" height="1" fill="#1e1e1e"/></svg>"##;

/// A 2x2 opaque white PNG.
fn white_png() -> Vec<u8> {
    fixture_png(2, 2, &[[255, 255, 255, 255]; 4])
}

/// A 2x2 opaque white PNG image element; the payload leaks so the fixture can
/// borrow it, once per test.
fn white_png_fixture(geometry: &str) -> ImageFixture<'_> {
    ImageFixture {
        media_type: "image/png",
        payload: Box::leak(white_png().into_boxed_slice()),
        geometry,
    }
}

fn nested_svg_fixture(geometry: &str) -> ImageFixture<'_> {
    ImageFixture {
        media_type: "image/svg+xml",
        payload: NESTED_SVG,
        geometry,
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

/// One image element and the file it references.
struct ImageFixture<'a> {
    media_type: &'a str,
    payload: &'a [u8],
    /// The element's `x`, `y`, `width`, `height` and any other member.
    geometry: &'a str,
}

/// A scene of image elements, each with its own file.
fn image_scene(images: &[ImageFixture<'_>]) -> SceneDocument {
    let elements = images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            format!(
                r#"{{"id":"image-{index}","type":"image","fileId":"file-{index}","status":"saved","angle":0,"opacity":100,"scale":[1,1],"seed":1,"updated":1,{}}}"#,
                image.geometry
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let files = images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            format!(
                r#""file-{index}":{{"id":"file-{index}","mimeType":"{0}","dataURL":"data:{0};base64,{1}","created":1}}"#,
                image.media_type,
                base64::engine::general_purpose::STANDARD.encode(image.payload)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{elements}],"appState":{{}},"files":{{{files}}}}}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}

fn defs(svg: &str) -> &str {
    let start = svg.find("<defs>").expect("SVG has <defs>");
    let end = svg.find("</defs>").expect("SVG closes <defs>");
    &svg[start..end]
}

/// The `<use>` element drawing the shared image keyed by `source_index`.
fn use_element(svg: &str, source_index: usize) -> &str {
    let href = format!("<use href=\"#image-{source_index}\" ");
    let start = svg
        .find(&href)
        .unwrap_or_else(|| panic!("SVG draws image-{source_index}: {svg}"));
    let end = svg[start..].find("/>").expect("use element closes") + start + 2;
    &svg[start..end]
}

/// The shared `<image>` element whose data URL carries `media_type`.
fn image_element<'a>(svg: &'a str, media_type: &str) -> &'a str {
    let href = format!("<image href=\"data:{media_type};base64,");
    let start = svg
        .find(&href)
        .unwrap_or_else(|| panic!("SVG has a {media_type} image: {svg}"));
    let end = svg[start..].find("/>").expect("image element closes") + start + 2;
    &svg[start..end]
}

fn data_url<'a>(svg: &'a str, media_type: &str) -> &'a str {
    let element = image_element(svg, media_type);
    let start = element.find("href=\"").expect("image has an href") + "href=\"".len();
    let end = element[start..].find('"').expect("href closes") + start;
    &element[start..end]
}

fn frame_outline<'a>(svg: &'a str, id: &str) -> &'a str {
    let marker = format!("<rect data-frame-outline=\"{id}\"");
    let start = svg
        .find(&marker)
        .unwrap_or_else(|| panic!("SVG has the {id} frame outline: {svg}"));
    let end = svg[start..].find("/>").expect("outline rect closes") + start;
    &svg[start..end]
}

fn request(format: OutputFormat, mode: ColorMode) -> RenderRequest {
    RenderRequest::builder(format)
        .color_mode(mode)
        .frame_options(FrameOptions::new(true, true, false, true))
        .build()
        .expect("request is valid")
}

fn render_outcome(
    scene: &SceneDocument,
    format: OutputFormat,
    mode: ColorMode,
) -> excalidraw_render::RenderOutcome {
    render_with_request(scene, &request(format, mode))
}

/// Renders a scene that is expected to need no diagnostic.
fn render(scene: &SceneDocument, format: OutputFormat, mode: ColorMode) -> Vec<u8> {
    artifact_without_diagnostics(&render_outcome(scene, format, mode))
}

fn render_png(scene: &SceneDocument, mode: ColorMode) -> DecodedPng {
    decode_png(&render(scene, OutputFormat::Png, mode))
}

/// Renders a scene that is expected to be diagnosed (a missing image).
fn render_png_with_diagnostics(scene: &SceneDocument, mode: ColorMode) -> DecodedPng {
    let outcome = render_outcome(scene, OutputFormat::Png, mode);
    assert!(
        !outcome.report().is_empty(),
        "the missing image is diagnosed"
    );
    decode_png(outcome.artifact().bytes())
}

fn rgba([red, green, blue]: [u8; 3]) -> [u8; 4] {
    [red, green, blue, 255]
}

fn assert_rgb_near(actual: [u8; 3], expected: [u8; 3]) {
    for (actual_channel, expected_channel) in actual.into_iter().zip(expected) {
        assert!(
            actual_channel.abs_diff(expected_channel) <= 1,
            "{actual:?} differs from {expected:?}"
        );
    }
}

/// The RGB stream of the first image XObject and the DeviceGray stream of its
/// soft mask.
fn image_xobject_streams(pdf: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let pdf = inflate_streams(pdf);
    let rgb = stream_after(&pdf, b"/ColorSpace /CS1\n/BitsPerComponent 8\n/SMask ");
    let alpha = stream_after(&pdf, b"/ColorSpace /DeviceGray\n/BitsPerComponent 8\n");
    (rgb, alpha)
}

/// The bytes of the inflated stream whose dictionary contains `marker`, sized
/// by its `/Length`.
fn stream_after(pdf: &[u8], marker: &[u8]) -> Vec<u8> {
    let dictionary = find_bytes(pdf, marker).expect("PDF has the image dictionary");
    let length_key = b"/Length ";
    let length_start = dictionary
        + find_bytes(&pdf[dictionary..], length_key).expect("dictionary has a length")
        + length_key.len();
    let length_end = length_start
        + pdf[length_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .expect("length ends");
    let length: usize = std::str::from_utf8(&pdf[length_start..length_end])
        .expect("length is ASCII")
        .parse()
        .expect("length is a number");
    let stream_marker = b"stream\n";
    let stream_start = length_end
        + find_bytes(&pdf[length_end..], stream_marker).expect("dictionary opens a stream")
        + stream_marker.len();
    pdf[stream_start..stream_start + length].to_vec()
}
