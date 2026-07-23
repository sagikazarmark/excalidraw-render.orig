use std::borrow::Cow;
use std::io::Cursor;

use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, OutputFormat, Recovery, RenderRequest, Renderer, ResourceLimits,
    SceneDocument,
};
use gif::{DisposalMethod, Encoder, Frame, Repeat};

#[test]
fn direct_svg_preserves_validated_gif_octets_in_a_canonical_data_url() {
    let gif = fixture_gif(&[frame(&[0, 1], 2, 1, &[255, 0, 0, 0, 0, 255], None)]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&gif);
    let outcome = render_outcome(&gif_scene(&gif, 2, 1), OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&format!(
        "<image href=\"data:image/gif;base64,{encoded}\" width=\"2\" height=\"1\"/>"
    )));
}

#[test]
fn png_projects_gif_palette_transparency_and_frame_offset_on_a_transparent_canvas() {
    let mut first = frame(&[1], 1, 1, &[0, 0, 255, 255, 0, 0], Some(0));
    first.left = 1;
    let gif = fixture_gif_with_screen(2, 1, &[first], None);
    let outcome = render_without_background(&gif_scene(&gif, 20, 10), OutputFormat::Png);
    let rgba = decoded_png(outcome.artifact().bytes());

    assert_eq!(pixel(&rgba, 40, 14, 15), [0, 0, 0, 0]);
    assert_eq!(pixel(&rgba, 40, 25, 15), [255, 0, 0, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn png_uses_the_first_gif_frame_and_reports_each_rendered_occurrence() {
    let gif = fixture_gif(&[
        frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], None),
        frame(&[1], 1, 1, &[255, 0, 0, 0, 0, 255], None),
    ]);
    let scene = two_gif_scene(&gif);

    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render_outcome(&scene, format);
        assert_eq!(outcome.report().diagnostics().len(), 2);
        for diagnostic in outcome.report().diagnostics() {
            assert_eq!(diagnostic.code(), DiagnosticCode::IMAGE_ANIMATION_FLATTENED);
            assert!(matches!(
                diagnostic.details(),
                Some(ErrorDetails::Asset {
                    validated_media_type: Some(media_type),
                    recovery: Recovery::Flattened,
                    ..
                }) if media_type == "image/gif"
            ));
        }
        if format == OutputFormat::Png {
            let rgba = decoded_png(outcome.artifact().bytes());
            assert_eq!(pixel(&rgba, 23, 10, 10), [255, 0, 0, 255]);
            assert_eq!(pixel(&rgba, 23, 12, 10), [255, 0, 0, 255]);
        }
    }

    let svg = render_outcome(&scene, OutputFormat::Svg);
    assert!(svg.report().diagnostics().is_empty());
}

#[test]
fn loop_metadata_does_not_make_a_single_frame_gif_animated() {
    let gif = fixture_gif_with_screen(
        1,
        1,
        &[frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], None)],
        Some(Repeat::Infinite),
    );

    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        assert!(render_outcome(&gif_scene(&gif, 1, 1), format)
            .report()
            .diagnostics()
            .is_empty());
    }
}

#[test]
fn direct_svg_validates_every_gif_frame_before_passthrough() {
    let mut gif = fixture_gif(&[
        frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], None),
        frame(&[1], 1, 1, &[255, 0, 0, 0, 0, 255], None),
    ]);
    gif.truncate(gif.len() - 3);

    let outcome = render_outcome(&gif_scene(&gif, 1, 1), OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_DECODE_FAILED
    );
    assert!(!std::str::from_utf8(outcome.artifact().bytes())
        .expect("SVG is UTF-8")
        .contains("<image "));
}

#[test]
fn gif87a_and_frame_local_palettes_are_supported() {
    let mut gif = fixture_gif(&[frame(&[1], 1, 1, &[0, 255, 0, 255, 0, 255], None)]);
    let control = gif
        .windows(3)
        .position(|window| window == [0x21, 0xf9, 0x04])
        .expect("fixture has a graphic control extension");
    gif.drain(control..control + 8);
    gif[..6].copy_from_slice(b"GIF87a");

    let outcome = render_outcome(&gif_scene(&gif, 1, 1), OutputFormat::Png);
    let rgba = decoded_png(outcome.artifact().bytes());

    assert_eq!(pixel(&rgba, 21, 10, 10), [255, 0, 255, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn malformed_palette_references_and_fixed_extension_blocks_are_rejected() {
    let palette = [255, 0, 0, 0, 0, 255];
    let global_frame = Frame {
        width: 1,
        height: 1,
        buffer: Cow::Owned(vec![0]),
        ..Frame::default()
    };
    let mut invalid_background = fixture_gif_with_global_palette(1, 1, &palette, &[global_frame]);
    invalid_background[11] = 2;

    let mut invalid_transparency =
        fixture_gif(&[frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], Some(0))]);
    let control = invalid_transparency
        .windows(3)
        .position(|window| window == [0x21, 0xf9, 0x04])
        .expect("fixture has a graphic control extension");
    invalid_transparency[control + 6] = 2;

    let mut malformed_application = fixture_gif_with_screen(
        1,
        1,
        &[frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], None)],
        Some(Repeat::Infinite),
    );
    let application = malformed_application
        .windows(2)
        .position(|window| window == [0x21, 0xff])
        .expect("fixture has an application extension");
    malformed_application[application + 2] = 10;

    for malformed in [
        invalid_background,
        invalid_transparency,
        malformed_application,
    ] {
        let outcome = render_outcome(&gif_scene(&malformed, 1, 1), OutputFormat::Svg);
        assert_eq!(outcome.report().diagnostics().len(), 1);
        assert_eq!(
            outcome.report().diagnostics()[0].code(),
            DiagnosticCode::IMAGE_DECODE_FAILED
        );
    }
}

#[test]
fn malformed_reserved_disposal_is_rejected() {
    let mut gif = fixture_gif(&[frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], None)]);
    let control = gif
        .windows(3)
        .position(|window| window == [0x21, 0xf9, 0x04])
        .expect("fixture has a graphic control extension");
    gif[control + 3] = 4 << 2;

    let outcome = render_outcome(&gif_scene(&gif, 1, 1), OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_DECODE_FAILED
    );
}

#[test]
fn malformed_image_descriptor_reserved_bits_are_rejected() {
    let mut gif = fixture_gif(&[frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], None)]);
    let descriptor = gif
        .iter()
        .position(|byte| *byte == 0x2c)
        .expect("fixture has an image descriptor");
    gif[descriptor + 9] |= 0x08;

    let outcome = render_outcome(&gif_scene(&gif, 1, 1), OutputFormat::Svg);

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_DECODE_FAILED
    );
}

#[test]
fn global_palette_interlacing_and_valid_disposal_modes_are_accepted() {
    let palette = [255, 0, 0, 0, 0, 255];
    let mut frames = Vec::new();
    for disposal in [
        DisposalMethod::Any,
        DisposalMethod::Keep,
        DisposalMethod::Background,
        DisposalMethod::Previous,
    ] {
        let mut frame = Frame {
            width: 2,
            height: 2,
            dispose: disposal,
            buffer: Cow::Owned(vec![0, 1, 1, 0]),
            ..Frame::default()
        };
        frame.interlaced = true;
        frames.push(frame);
    }
    let gif = fixture_gif_with_global_palette(2, 2, &palette, &frames);

    let svg = render_outcome(&gif_scene(&gif, 20, 20), OutputFormat::Svg);
    assert!(svg.report().diagnostics().is_empty());
    let png = render_outcome(&gif_scene(&gif, 20, 20), OutputFormat::Png);
    assert_eq!(
        png.report().diagnostics()[0].code(),
        DiagnosticCode::IMAGE_ANIMATION_FLATTENED
    );
    assert_eq!(
        pixel(&decoded_png(png.artifact().bytes()), 40, 14, 14),
        [255, 0, 0, 255]
    );
}

#[test]
fn animation_validation_work_uses_additional_frame_rectangles() {
    let mut second = frame(&[1], 1, 1, &[255, 0, 0, 0, 0, 255], None);
    second.left = 1;
    second.top = 1;
    let gif = fixture_gif_with_screen(
        2,
        2,
        &[
            frame(&[0, 0, 0, 0], 2, 2, &[255, 0, 0, 0, 0, 255], None),
            second,
        ],
        None,
    );
    let payload_bytes = u64::try_from(gif.len()).expect("fixture length fits u64");

    for (limit, expected) in [(payload_bytes + 4, 0), (payload_bytes + 3, 1)] {
        let limits = ResourceLimits::builder()
            .max_asset_work_units(limit)
            .build()
            .expect("asset work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("SVG request is valid");
        let outcome = Renderer::new()
            .render(&gif_scene(&gif, 2, 2), &request)
            .expect("GIF validation has local recovery");
        assert_eq!(outcome.report().diagnostics().len(), expected);
        if expected == 1 {
            assert_eq!(
                outcome.report().diagnostics()[0].code(),
                DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED
            );
        }
    }
}

#[test]
fn flattening_is_not_reported_when_png_local_recovery_omits_the_occurrence() {
    let gif = fixture_gif(&[
        frame(&[0], 1, 1, &[255, 0, 0, 0, 0, 255], None),
        frame(&[1], 1, 1, &[255, 0, 0, 0, 0, 255], None),
    ]);
    let limits = ResourceLimits::builder()
        .max_element_work_units(31)
        .build()
        .expect("element work limit is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(limits)
        .build()
        .expect("PNG request is valid");

    let outcome = Renderer::new()
        .render(&gif_scene(&gif, 1, 1), &request)
        .expect("PNG local omission recovers");

    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].code(),
        DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
    );
}

fn frame<'a>(
    indices: &'a [u8],
    width: u16,
    height: u16,
    palette: &'a [u8],
    transparent: Option<u8>,
) -> Frame<'a> {
    Frame {
        width,
        height,
        palette: Some(palette.to_vec()),
        transparent,
        dispose: DisposalMethod::Keep,
        buffer: Cow::Borrowed(indices),
        ..Frame::default()
    }
}

fn fixture_gif(frames: &[Frame<'_>]) -> Vec<u8> {
    fixture_gif_with_screen(frames[0].width, frames[0].height, frames, None)
}

fn fixture_gif_with_screen(
    width: u16,
    height: u16,
    frames: &[Frame<'_>],
    repeat: Option<Repeat>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, width, height, &[]).expect("GIF header encodes");
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat).expect("GIF repeat encodes");
        }
        for frame in frames {
            encoder.write_frame(frame).expect("GIF frame encodes");
        }
    }
    bytes
}

fn fixture_gif_with_global_palette(
    width: u16,
    height: u16,
    palette: &[u8],
    frames: &[Frame<'_>],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder =
            Encoder::new(&mut bytes, width, height, palette).expect("GIF header encodes");
        for frame in frames {
            encoder.write_frame(frame).expect("GIF frame encodes");
        }
    }
    bytes
}

fn gif_scene(gif: &[u8], width: u32, height: u32) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(gif);
    image_scene(
        &format!("data:image/gif;base64,{encoded}"),
        "image/gif",
        width,
        height,
    )
}

fn two_gif_scene(gif: &[u8]) -> SceneDocument {
    let encoded = base64::engine::general_purpose::STANDARD.encode(gif);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"one","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
                {{"id":"two","type":"image","x":2,"y":0,"width":1,"height":1,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
            ],
            "appState":{{}},"files":{{
                "one":{{"id":"one","mimeType":"image/gif","dataURL":"data:image/gif;base64,{encoded}","created":1}},
                "two":{{"id":"two","mimeType":"image/gif","dataURL":"data:image/gif;base64,{encoded}","created":1}}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("two-GIF scene is admitted")
}

fn image_scene(data_url: &str, media_type: &str, width: u32, height: u32) -> SceneDocument {
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[{{
                "id":"image","type":"image","x":0,"y":0,
                "width":{width},"height":{height},"angle":0,"opacity":100,
                "fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1
            }}],
            "appState":{{}},
            "files":{{"asset":{{
                "id":"asset","mimeType":"{media_type}",
                "dataURL":"{data_url}","created":1
            }}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}

fn render_outcome(scene: &SceneDocument, format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("render request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("GIF-family asset has local recovery")
}

fn render_without_background(
    scene: &SceneDocument,
    format: OutputFormat,
) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .include_background(false)
        .build()
        .expect("render request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("GIF-family asset renders")
}

fn decoded_png(bytes: &[u8]) -> Vec<u8> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut rgba).expect("PNG frame decodes");
    rgba
}

fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    rgba[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .expect("pixel has four channels")
}
