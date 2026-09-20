//! The fixed Stage-4 typography crop is also a focused PNG regression. This
//! checks reference pixels, independently of the candidate's approval status.
use std::io::Cursor;

use excalidraw_render::{FrameOptions, OutputFormat, RenderRequest, Renderer, SceneDocument, Sizing};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};

#[test]
fn rotated_canvas_glyph_origins_match_the_reference_typography_crop() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/candidates/stage4-frames-text-assets-v4");
    let scene = SceneDocument::parse(&std::fs::read(root.join("scene.json")).unwrap()).unwrap();
    let request = RenderRequest::builder(OutputFormat::Png)
        .padding(0.0)
        .include_background(false)
        .frame_options(FrameOptions::new(true, true, false, false))
        .build()
        .unwrap();
    let result = Renderer::new().render(&scene, &request).unwrap();
    let mut decoder = png::Decoder::new(Cursor::new(result.artifact().bytes()));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut actual = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut actual).unwrap();
    assert_eq!((info.width, info.height), (200, 120));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    actual.truncate(info.buffer_size());
    let reference = std::fs::read(root.join("official-png.rgba")).unwrap();
    let crop = |rgba: &[u8]| {
        let mut cropped = Vec::new();
        for y in 12..53 {
            cropped.extend_from_slice(&rgba[(y * 200 + 10) * 4..(y * 200 + 134) * 4]);
        }
        CanonicalRgba8::new(124, 41, cropped).unwrap()
    };
    let comparison = compare(
        &crop(&reference),
        &crop(&actual),
        VisualClass::Typography,
        &[],
        ComparisonMode::Canonical,
    )
    .unwrap();
    assert!(
        comparison.passed(),
        "changed={} ssim={:?}",
        comparison.whole.changed_pixels,
        comparison.whole.ssim
    );
}

// Independent placement controls, authored in memory. The fixed v4 files are
// only a source of admitted element fields; their scene/crops are never edited.
fn phase_control(font_size: f64, angle: f64, origin: [f64; 2]) -> Vec<u8> {
    phase_control_scaled(font_size, angle, 1.0, origin)
}

fn phase_control_scaled(font_size: f64, angle: f64, scale: f64, origin: [f64; 2]) -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/candidates/stage4-frames-text-assets-v4");
    let mut scene: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("scene.json")).unwrap()).unwrap();
    let elements = scene["elements"].as_array().unwrap();
    let mut frame = elements
        .iter()
        .find(|e| e["id"] == "frame")
        .unwrap()
        .clone();
    let mut text = elements
        .iter()
        .find(|e| e["id"] == "caption")
        .unwrap()
        .clone();
    frame["width"] = serde_json::json!(512.0 / scale);
    frame["height"] = serde_json::json!(512.0 / scale);
    let origin = [origin[0] / scale, origin[1] / scale];
    let width = font_size;
    let height = font_size * 1.25;
    // Excalifont family metrics: 886 ascent, -374 descent, UPEM 1000;
    // 1.25 line height gives baseline (886 + (1250 - 1260)/2)/1000.
    let baseline = font_size * 0.881;
    let (sin, cos) = angle.sin_cos();
    let x = origin[0] + sin * baseline - width / 2.0 + cos * width / 2.0 - sin * height / 2.0;
    let y = origin[1] - cos * baseline - height / 2.0 + sin * width / 2.0 + cos * height / 2.0;
    for (key, value) in [
        ("x", x),
        ("y", y),
        ("width", width),
        ("height", height),
        ("fontSize", font_size),
        ("angle", angle),
    ] {
        text[key] = serde_json::json!(value);
    }
    text["text"] = "S".into();
    text["originalText"] = "S".into();
    scene["elements"] = serde_json::json!([frame, text]);
    let scene = SceneDocument::parse(&serde_json::to_vec(&scene).unwrap()).unwrap();
    let request = RenderRequest::builder(OutputFormat::Png)
        .padding(0.0)
        .include_background(false)
        .sizing(Sizing::Scale(scale))
        .frame_options(FrameOptions::new(true, true, false, false))
        .build()
        .unwrap();
    let result = Renderer::new().render(&scene, &request).unwrap();
    let mut decoder = png::Decoder::new(Cursor::new(result.artifact().bytes()));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).unwrap();
    assert_eq!((info.width, info.height), (512, 512));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    rgba.truncate(info.buffer_size());
    assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    rgba
}

#[test]
fn quarter_pixel_halfway_ties_choose_the_positive_device_coordinate() {
    let tie = phase_control(20.0, 0.0, [50.125, 300.1]);
    let upper = phase_control(20.0, 0.0, [50.25, 300.1]);
    let lower = phase_control(20.0, 0.0, [50.0, 300.1]);
    assert!(
        tie == upper,
        "eighth-pixel tie must select the next quarter pixel"
    );
    assert!(
        tie != lower,
        "tie must not round toward zero or to the lower phase"
    );
}

#[test]
fn canvas_mask_cutoff_is_strictly_above_256_device_pixels() {
    for (size, snapped) in [(20.0, true), (256.0, true), (256.25, false)] {
        let first = phase_control(size, 0.0, [50.1, 300.1]);
        let shifted = phase_control(size, 0.0, [50.1, 300.35]);
        assert_eq!(first == shifted, snapped, "font size {size}");
    }
}

#[test]
fn quarter_turn_text_snaps_the_perpendicular_device_axis() {
    for angle in [std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2] {
        assert_phase_axis(20.0, angle, 1.0, [true, false]);
    }
    assert_phase_axis(20.0, std::f64::consts::PI, 1.0, [false, true]);
}

// Expectations below were independently observed in pinned Chromium 149,
// two fresh processes, using the same one-glyph device-origin displacements.
// true means the two raw RGBA buffers are identical, not a visual tolerance.
fn assert_phase_axis(font_size: f64, angle: f64, scale: f64, identical: [bool; 2]) {
    let first = phase_control_scaled(font_size, angle, scale, [50.1, 300.1]);
    for (axis, origin) in [[50.35, 300.1], [50.1, 300.35]].into_iter().enumerate() {
        let shifted = phase_control_scaled(font_size, angle, scale, origin);
        let changed = first
            .chunks_exact(4)
            .zip(shifted.chunks_exact(4))
            .filter(|(left, right)| left != right)
            .count();
        assert_eq!(
            changed == 0,
            identical[axis],
            "size={font_size} angle={angle} scale={scale} axis={axis}: {changed} changed pixels"
        );
    }
}

#[test]
fn font_cache_alignment_matches_pinned_signed_threshold_controls() {
    // Adjacent binary32 coefficients either side of +1/2048. The source
    // rounds ties toward +infinity: +1/2048 is outside, -1/2048 is inside.
    let half = 1.0 / 2048.0;
    let inside = half - 1.0 / 34_359_738_368.0; // 2^-35
    let outside = half + 1.0 / 17_179_869_184.0; // 2^-34
    for (coefficient, positive_aligned, negative_aligned) in [
        (inside, true, true),
        (half, false, true),
        (outside, false, false),
    ] {
        for (sign, aligned) in [(1.0, positive_aligned), (-1.0, negative_aligned)] {
            let value: f64 = sign * coefficient;
            assert_phase_axis(20.0, value.asin(), 1.0, [false, aligned]);
            assert_phase_axis(20.0, value.acos(), 1.0, [aligned, false]);
        }
    }
    // These f64 coefficients become exact signed ties on admission to SkScalar.
    // Testing the f64 coefficient directly would choose the opposite phase grid.
    let positive: f64 = half - 1.0 / 137_438_953_472.0; // 2^-37
    let negative: f64 = -half - 1.0 / 68_719_476_736.0; // 2^-36
    assert_phase_axis(20.0, positive.asin(), 1.0, [false, false]);
    assert_phase_axis(20.0, negative.asin(), 1.0, [false, true]);
}

#[test]
fn alignment_uses_export_matrix_not_font_size_or_supersample() {
    let inside: f64 = 1.0 / 2048.0 - 1.0 / 34_359_738_368.0;
    let outside: f64 = 1.0 / 2048.0 + 1.0 / 17_179_869_184.0;
    assert_phase_axis(20.0, 0.0001, 1.0, [false, true]);
    assert_phase_axis(20.0, -0.0001, 1.0, [false, true]);
    assert_phase_axis(20.0, 0.125, 1.0, [false, false]);
    assert_phase_axis(80.0, inside.asin(), 1.0, [false, true]);
    assert_phase_axis(80.0, outside.asin(), 1.0, [false, false]);
    assert_phase_axis(20.0, inside.asin(), 2.0, [false, false]);
    assert_phase_axis(20.0, (inside / 2.0).asin(), 2.0, [false, true]);
    assert_phase_axis(20.0, (outside / 2.0).asin(), 2.0, [false, false]);
    assert_phase_axis(128.0, 0.0, 2.0, [false, true]);
    assert_phase_axis(128.125, 0.0, 2.0, [false, false]);
}
