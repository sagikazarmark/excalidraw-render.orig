use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorCode, ErrorDetails, OutputFormat, Recovery, RenderRequest, Renderer,
    ResourceLimits, SceneDocument, SupplementalAssets,
};
use std::io::Cursor;

const PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

#[test]
fn direct_svg_preserves_validated_static_png_octets_in_a_canonical_data_url() {
    let scene = image_scene(&format!("data:image/png;base64,{PNG_BASE64}"));
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("a referenced static PNG renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains(&format!(
        "<image href=\"data:image/png;base64,{PNG_BASE64}\" width=\"1\" height=\"1\"/>"
    )));
}

#[test]
fn png_projects_canonical_rgba_pixels_into_the_image_occurrence() {
    let scene = image_scene(&format!("data:image/png;base64,{PNG_BASE64}"));
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("PNG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("a referenced static PNG renders");
    let decoder = png::Decoder::new(Cursor::new(outcome.artifact().bytes()));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut pixels).expect("PNG frame decodes");

    assert_eq!((output.width, output.height), (21, 21));
    let center = (10 * 21 + 10) * 4;
    assert_eq!(&pixels[center..center + 4], &[0, 0, 0, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn apng_is_preserved_in_svg_and_flattened_to_frame_zero_in_png_and_pdf() {
    let apng = fixture_apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&apng);
    let scene = image_scene(&format!("data:image/png;base64,{encoded}"));

    let svg_outcome = render_outcome(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(&format!("data:image/png;base64,{encoded}")));
    assert!(svg_outcome.report().diagnostics().is_empty());

    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render_outcome(&scene, format);
        assert_eq!(
            diagnostic_codes(&outcome),
            [DiagnosticCode::IMAGE_ANIMATION_FLATTENED]
        );
        assert!(matches!(
            outcome.report().diagnostics()[0].details(),
            Some(ErrorDetails::Asset {
                asset_id,
                recovery: Recovery::Flattened,
                recipe_id: None,
                ..
            }) if asset_id == "asset"
        ));
        assert_eq!(
            outcome.report().diagnostics()[0].effects(),
            &[excalidraw_render::DiagnosticEffect::Visual]
        );
        assert_eq!(
            outcome.report().diagnostics()[0].json_pointer(),
            "/elements/0/fileId"
        );
        if format == OutputFormat::Png {
            let decoder = png::Decoder::new(Cursor::new(outcome.artifact().bytes()));
            let mut reader = decoder.read_info().expect("artifact is a PNG");
            let mut rgba = vec![0; reader.output_buffer_size()];
            reader.next_frame(&mut rgba).expect("PNG frame decodes");
            assert_eq!(pixel(&rgba, 21, 10, 10), [255, 0, 0, 255]);
        }
    }
}

#[test]
fn one_frame_apng_does_not_report_animation_flattening() {
    let mut apng = fixture_png(1, 1, &[[255, 0, 0, 255]]);
    apng = png_with_chunks_before_idat(
        &apng,
        &[
            (
                *b"acTL",
                [1_u32.to_be_bytes(), 0_u32.to_be_bytes()].concat(),
            ),
            (*b"fcTL", apng_frame_control(0, 1, 1)),
        ],
    );
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(apng)
    ));

    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render_outcome(&scene, format);
        assert!(outcome.report().diagnostics().is_empty());
    }
}

#[test]
fn shared_apng_bytes_report_each_rendered_occurrence() {
    let apng = fixture_apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let scene = two_payload_scene(&apng, &apng);

    let outcome = render_outcome(&scene, OutputFormat::Png);

    assert_eq!(
        diagnostic_codes(&outcome),
        [
            DiagnosticCode::IMAGE_ANIMATION_FLATTENED,
            DiagnosticCode::IMAGE_ANIMATION_FLATTENED,
        ]
    );
    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.element_id())
            .collect::<Vec<_>>(),
        [Some("one"), Some("two")]
    );
}

#[test]
fn apng_validation_charges_each_additional_frame_without_deriving_svg_pixels() {
    let apng = fixture_apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let payload_bytes = u64::try_from(apng.len()).expect("fixture length fits u64");
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(apng)
    ));

    for (limit, expected_diagnostics) in [(payload_bytes + 4, 0), (payload_bytes + 3, 1)] {
        let limits = ResourceLimits::builder()
            .max_asset_work_units(limit)
            .max_decoded_pixels_per_asset(1)
            .max_total_decoded_pixels(1)
            .build()
            .expect("APNG validation limits are valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("SVG request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("APNG validation has local resource recovery");

        assert_eq!(outcome.report().diagnostics().len(), expected_diagnostics);
        if expected_diagnostics == 1 {
            assert_eq!(
                outcome.report().diagnostics()[0].code(),
                DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED
            );
        }
    }
}

#[test]
fn png_bilinear_projection_charges_twelve_samples_per_occurrence_pixel() {
    let scene = image_scene(&format!("data:image/png;base64,{PNG_BASE64}"));
    for (limit, expected_diagnostics) in [(32, 0), (31, 1)] {
        let limits = ResourceLimits::builder()
            .max_element_work_units(limit)
            .build()
            .expect("element-work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Png)
            .resource_limits(limits)
            .build()
            .expect("PNG request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("image projection has local resource recovery");

        assert_eq!(outcome.report().diagnostics().len(), expected_diagnostics);
        if expected_diagnostics == 1 {
            assert_eq!(
                outcome.report().diagnostics()[0].code(),
                DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
            );
        }
    }
}

#[test]
fn png_image_local_work_includes_projection_mask_and_composite_samples() {
    let scene = image_scene(&format!("data:image/png;base64,{PNG_BASE64}"));
    for (limit, expected_diagnostics) in [(32, 0), (31, 1)] {
        let limits = ResourceLimits::builder()
            .max_element_work_units(limit)
            .build()
            .expect("element-work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Png)
            .resource_limits(limits)
            .build()
            .expect("PNG request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("image raster work has local recovery");

        assert_eq!(outcome.report().diagnostics().len(), expected_diagnostics);
        if expected_diagnostics == 1 {
            assert_eq!(
                outcome.report().diagnostics()[0].code(),
                DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
            );
        }
    }
}

#[test]
fn pdf_embeds_static_png_as_a_lossless_rgb_image_with_a_soft_mask() {
    let scene = image_scene(&format!("data:image/png;base64,{PNG_BASE64}"));
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("a referenced static PNG renders");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/ColorSpace /CS1"));
    assert!(pdf.contains("/ColorSpace /DeviceGray"));
    assert!(pdf.contains("/SMask "));
    assert!(pdf.contains("/Im0 Do"));
    assert!(!pdf.contains("pdf.raster-fallback"));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn supplemental_asset_diagnostics_sort_by_file_id_not_reference_order() {
    let scene = two_supplemental_image_scene("z-file", "a-file");
    let assets = SupplementalAssets::builder()
        .asset("z-file", b"not png z".to_vec())
        .asset("a-file", b"not png a".to_vec())
        .build()
        .expect("supplemental mapping is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("unsupported supplements have local recovery");
    let asset_ids: Vec<&str> = outcome
        .report()
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| match diagnostic.details() {
            Some(ErrorDetails::Asset { asset_id, .. }) => Some(asset_id.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(asset_ids, ["a-file", "z-file"]);
}

#[test]
fn scene_payload_remains_authoritative_over_a_conflicting_supplement() {
    let scene = image_scene(&format!("data:image/png;base64,{PNG_BASE64}"));
    let assets = SupplementalAssets::builder()
        .asset("asset", b"not the scene PNG".to_vec())
        .build()
        .expect("supplemental mapping is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("the authoritative scene PNG renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(svg.contains(&format!("data:image/png;base64,{PNG_BASE64}")));
    assert_eq!(
        diagnostic_codes(&outcome),
        [DiagnosticCode::ASSET_SUPPLEMENT_CONFLICT]
    );
}

#[test]
fn malformed_scene_record_still_blocks_a_same_id_supplement() {
    let scene = SceneDocument::parse(
        br#"{
            "type":"excalidraw","version":2,
            "elements":[{"id":"image","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1}],
            "appState":{},"files":{"asset":null}
        }"#,
    )
    .expect("malformed file record remains admitted scene content");
    let png = base64::engine::general_purpose::STANDARD
        .decode(PNG_BASE64)
        .expect("fixture base64 is valid");
    let assets = SupplementalAssets::builder()
        .asset("asset", png)
        .build()
        .expect("supplemental mapping is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("malformed authoritative content has local recovery");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(!svg.contains("<image "));
    assert_eq!(
        diagnostic_codes(&outcome),
        [
            DiagnosticCode::ASSET_MALFORMED_FILE_RECORD,
            DiagnosticCode::ASSET_SUPPLEMENT_CONFLICT,
        ]
    );
}

#[test]
fn supplemental_only_png_fills_the_referenced_file_id() {
    let scene = image_scene_without_files();
    let png = base64::engine::general_purpose::STANDARD
        .decode(PNG_BASE64)
        .expect("fixture base64 is valid");
    let assets = SupplementalAssets::builder()
        .asset("asset", png)
        .build()
        .expect("supplemental mapping is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("the supplemental PNG renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(svg.contains(&format!("data:image/png;base64,{PNG_BASE64}")));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn referenced_non_png_and_corrupt_png_supplements_use_typed_recovery() {
    for (bytes, expected) in [
        (
            b"not a png".to_vec(),
            DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT,
        ),
        (
            b"\x89PNG\r\n\x1a\ncorrupt".to_vec(),
            DiagnosticCode::IMAGE_DECODE_FAILED,
        ),
    ] {
        let assets = SupplementalAssets::builder()
            .asset_with_media_type("asset", bytes, "image/png")
            .build()
            .expect("supplemental mapping is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .supplemental_assets(assets)
            .build()
            .expect("SVG request is valid");

        let outcome = Renderer::new()
            .render(&image_scene_without_files(), &request)
            .expect("invalid image has local recovery");

        assert_eq!(diagnostic_codes(&outcome), [expected]);
        assert_eq!(outcome.report().diagnostics()[0].json_pointer(), "");
    }
}

#[test]
fn sniffed_png_type_wins_over_a_scene_media_claim() {
    let scene =
        image_scene_with_media_type(&format!("data:image/png;base64,{PNG_BASE64}"), "image/jpeg");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("the sniffed PNG renders");
    let [diagnostic] = outcome.report().diagnostics() else {
        panic!("one media mismatch is reported");
    };

    assert_eq!(diagnostic.code(), DiagnosticCode::ASSET_MEDIA_TYPE_MISMATCH);
    assert_eq!(diagnostic.json_pointer(), "/files/asset/mimeType");
    assert!(matches!(
        diagnostic.details(),
        Some(ErrorDetails::Asset {
            claimed_media_type: Some(claimed),
            validated_media_type: Some(validated),
            ..
        }) if claimed == "image/jpeg" && validated == "image/png"
    ));
}

#[test]
fn identical_png_bytes_keep_each_occurrence_media_claim_and_owner() {
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"one","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
                {{"id":"two","type":"image","x":2,"y":0,"width":1,"height":1,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
            ],
            "appState":{{}},"files":{{
                "one":{{"id":"one","mimeType":"image/jpeg","dataURL":"data:image/png;base64,{PNG_BASE64}","created":1}},
                "two":{{"id":"two","mimeType":"image/png","dataURL":"data:image/jpeg;base64,{PNG_BASE64}","created":1}}
            }}
        }}"#
    );
    let scene = SceneDocument::parse(json.as_bytes()).expect("claim scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("validated shared PNG bytes render");

    assert_eq!(
        outcome
            .report()
            .diagnostics()
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
            .collect::<Vec<_>>(),
        [
            (
                DiagnosticCode::ASSET_MEDIA_TYPE_MISMATCH,
                "/files/one/mimeType"
            ),
            (
                DiagnosticCode::ASSET_MEDIA_TYPE_MISMATCH,
                "/files/two/dataURL"
            ),
        ]
    );
    let claims = outcome
        .report()
        .diagnostics()
        .iter()
        .map(|diagnostic| match diagnostic.details() {
            Some(ErrorDetails::Asset {
                claimed_media_type: Some(claim),
                validated_media_type: Some(validated),
                ..
            }) => (claim.as_str(), validated.as_str()),
            _ => panic!("media mismatch has claimed and validated types"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        claims,
        [("image/jpeg", "image/png"), ("image/jpeg", "image/png")]
    );
}

#[test]
fn unreferenced_png_like_records_do_not_participate_in_validation() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{
            "unused":{"id":"unused","mimeType":"image/jpeg","dataURL":"data:image/png;base64,not-base64","created":1}
        }}"#,
    )
    .expect("scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("unreferenced payload does not affect rendering");

    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn duplicate_semantic_png_bytes_share_one_pdf_resource_but_render_twice() {
    let scene = two_image_scene();
    let request = RenderRequest::builder(OutputFormat::Pdf)
        .build()
        .expect("PDF request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("both PNG occurrences render");
    let pdf = String::from_utf8_lossy(outcome.artifact().bytes());

    assert_eq!(pdf.matches("/ColorSpace /CS1").count(), 1);
    assert_eq!(pdf.matches("/Im0 Do").count(), 2);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn semantic_pixel_work_shares_but_payload_occurrence_admission_does_not() {
    let scene = two_image_scene();
    let shared_limits = ResourceLimits::builder()
        .max_decoded_pixels_per_asset(1)
        .max_total_decoded_pixels(1)
        .build()
        .expect("shared pixel limit is valid");
    let shared_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(shared_limits)
        .build()
        .expect("SVG request is valid");

    Renderer::new()
        .render(&scene, &shared_request)
        .expect("identical PNG payloads share decoded pixels");

    let occurrence_limits = ResourceLimits::builder()
        .max_asset_entries(1)
        .build()
        .expect("occurrence limit is valid");
    let occurrence_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(occurrence_limits)
        .build()
        .expect("SVG request is valid");
    let error = Renderer::new()
        .render(&scene, &occurrence_request)
        .expect_err("each payload occurrence is admitted independently");

    assert_eq!(error.code(), ErrorCode::RESOURCE_ASSET_COUNT_EXCEEDED);
}

#[test]
fn aggregate_asset_bytes_and_distinct_decoded_pixels_cross_before_second_decode() {
    let red = fixture_png(1, 1, &[[255, 0, 0, 255]]);
    let blue = fixture_png(1, 1, &[[0, 0, 255, 255]]);
    let scene = two_payload_scene(&red, &blue);
    let total_bytes = u64::try_from(red.len() + blue.len()).expect("fixture bytes fit u64");
    let byte_limits = ResourceLimits::builder()
        .max_asset_bytes(u64::try_from(red.len().max(blue.len())).expect("fixture length fits u64"))
        .max_total_asset_bytes(total_bytes - 1)
        .build()
        .expect("asset limits are valid");
    let byte_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(byte_limits)
        .build()
        .expect("SVG request is valid");
    let error = Renderer::new()
        .render(&scene, &byte_request)
        .expect_err("aggregate bytes reject the second payload occurrence");
    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_ASSET_BYTES_EXCEEDED);

    let pixel_limits = ResourceLimits::builder()
        .max_decoded_pixels_per_asset(1)
        .max_total_decoded_pixels(1)
        .build()
        .expect("pixel limits are valid");
    let pixel_request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(pixel_limits)
        .build()
        .expect("PNG request is valid");
    let error = Renderer::new()
        .render(&scene, &pixel_request)
        .expect_err("distinct second PNG crosses aggregate decoded pixels before decode");
    assert_eq!(
        error.code(),
        ErrorCode::RESOURCE_TOTAL_DECODED_PIXELS_EXCEEDED
    );
}

#[test]
fn asset_byte_limit_recovers_before_static_png_validation_or_decode() {
    let png = fixture_png(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    ));
    let limits = ResourceLimits::builder()
        .max_asset_bytes(u64::try_from(png.len() - 1).expect("fixture length fits u64"))
        .build()
        .expect("asset-byte limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("local asset-byte excess has format-specific recovery");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert_eq!(
        diagnostic_codes(&outcome),
        [DiagnosticCode::RESOURCE_ASSET_BYTES_EXCEEDED]
    );
    assert!(!svg.contains("<image "));
}

#[test]
fn png_dimension_pixel_and_work_limits_recover_from_ihdr_before_decode() {
    let png = fixture_png(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    ));
    let cases = [
        (
            ResourceLimits::builder().max_raster_dimension(1),
            DiagnosticCode::RESOURCE_IMAGE_DIMENSIONS_EXCEEDED,
        ),
        (
            ResourceLimits::builder()
                .max_decoded_pixels_per_asset(1)
                .max_total_decoded_pixels(1),
            DiagnosticCode::RESOURCE_IMAGE_PIXELS_EXCEEDED,
        ),
        (
            ResourceLimits::builder().max_asset_work_units(1),
            DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED,
        ),
    ];

    for (limits, expected) in cases {
        let request = RenderRequest::builder(OutputFormat::Png)
            .resource_limits(limits.build().expect("resource limits are valid"))
            .build()
            .expect("PNG request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("local static PNG limit has format-specific recovery");
        assert_eq!(diagnostic_codes(&outcome), [expected]);
    }
}

#[test]
fn direct_svg_validates_static_png_without_deriving_decoded_pixels() {
    let png = fixture_png(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    ));
    let limits = ResourceLimits::builder()
        .max_raster_dimension(1)
        .max_decoded_pixels_per_asset(1)
        .max_total_decoded_pixels(1)
        .build()
        .expect("resource limits are valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("direct SVG validates compressed PNG without pixel derivation");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains("<image "));
}

#[test]
fn direct_svg_rejects_corrupt_idat_with_a_valid_chunk_crc() {
    let mut png = fixture_png(1, 1, &[[255, 0, 0, 255]]);
    mutate_first_idat(&mut png, |data| {
        data[0] ^= 0xff;
    });
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ));
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("corrupt image data has local recovery");

    assert_eq!(
        diagnostic_codes(&outcome),
        [DiagnosticCode::IMAGE_DECODE_FAILED]
    );
}

#[test]
fn direct_svg_validates_every_apng_frame_before_passthrough() {
    let mut apng = fixture_apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    mutate_first_png_chunk(&mut apng, *b"fdAT", |data| {
        data[4] ^= 0xff;
    });
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(apng)
    ));

    let outcome = render_outcome(&scene, OutputFormat::Svg);

    assert_eq!(
        diagnostic_codes(&outcome),
        [DiagnosticCode::IMAGE_DECODE_FAILED]
    );
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(!svg.contains("<image "));
}

#[test]
fn direct_svg_validates_animation_after_a_separate_apng_default_image() {
    let apng = apng_with_separate_default(true);
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(apng)
    ));

    let outcome = render_outcome(&scene, OutputFormat::Svg);

    assert_eq!(
        diagnostic_codes(&outcome),
        [DiagnosticCode::IMAGE_DECODE_FAILED]
    );
}

#[test]
fn png_selects_animation_frame_zero_instead_of_a_separate_apng_default_image() {
    let apng = apng_with_separate_default(false);

    assert_eq!(rendered_image_center(&apng), [0, 0, 255, 255]);
}

#[test]
fn png_normalizes_gama_to_srgb_and_srgb_signaling_takes_precedence() {
    let source = fixture_png(1, 1, &[[128, 128, 128, 255]]);
    let linear =
        png_with_chunks_before_idat(&source, &[(*b"gAMA", 100_000_u32.to_be_bytes().to_vec())]);
    let srgb = png_with_chunks_before_idat(
        &source,
        &[
            (*b"gAMA", 100_000_u32.to_be_bytes().to_vec()),
            (*b"sRGB", vec![0]),
        ],
    );

    assert_eq!(rendered_image_center(&linear), [188, 188, 188, 255]);
    assert_eq!(rendered_image_center(&srgb), [128, 128, 128, 255]);
}

#[test]
fn png_cicp_takes_precedence_over_legacy_color_chunks() {
    let source = fixture_png(1, 1, &[[128, 128, 128, 255]]);
    let tagged = png_with_chunks_before_idat(
        &source,
        &[
            (*b"gAMA", 100_000_u32.to_be_bytes().to_vec()),
            (*b"cICP", vec![1, 13, 0, 1]),
        ],
    );

    assert_eq!(rendered_image_center(&tagged), [128, 128, 128, 255]);
}

#[test]
fn png_standard_sdr_projection_uses_the_gain_map_base_image() {
    let source = fixture_png(1, 1, &[[64, 128, 192, 255]]);
    let gain_map = fixture_png(1, 1, &[[255, 255, 255, 255]]);
    let tagged = png_with_chunks_before_idat(
        &source,
        &[(*b"gmAP", iso_gain_map_metadata()), (*b"gdAT", gain_map)],
    );

    assert_eq!(rendered_image_center(&tagged), [64, 128, 192, 255]);
}

#[test]
fn png_canonicalizes_alpha_through_premultiplied_canvas_storage() {
    let partial = fixture_png(1, 1, &[[200, 100, 50, 128]]);
    let transparent = fixture_png(1, 1, &[[255, 17, 33, 0]]);

    assert_eq!(
        rendered_image_center_without_background(&partial),
        [199, 100, 50, 128]
    );
    assert_eq!(
        rendered_image_center_without_background(&transparent),
        [0, 0, 0, 0]
    );
}

#[test]
fn png_tone_maps_hdr_cicp_instead_of_treating_samples_as_srgb() {
    let source = fixture_png(1, 1, &[[128, 128, 128, 255]]);
    let hdr = png_with_chunks_before_idat(&source, &[(*b"cICP", vec![9, 16, 0, 1])]);
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&hdr)
    ));
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("PNG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("unsupported HDR signaling has local recovery");

    assert!(outcome.report().diagnostics().is_empty());
    assert_eq!(rendered_image_center(&hdr), [24, 24, 24, 255]);
}

#[test]
fn png_and_pdf_apply_png_exif_orientation_while_svg_preserves_source_bytes() {
    let source = fixture_png(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let exif = exif_orientation(6);
    for oriented in [
        png_with_chunks_before_idat(&source, &[(*b"eXIf", exif.clone())]),
        png_with_chunk_before_iend(&source, *b"eXIf", &exif),
    ] {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&oriented);
        let scene = image_scene_with_size(&format!("data:image/png;base64,{encoded}"), 10, 20);

        let svg_outcome = render_outcome(&scene, OutputFormat::Svg);
        let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
        assert!(svg.contains(&format!("data:image/png;base64,{encoded}")));

        let png = render_artifact(&scene, OutputFormat::Png);
        let decoder = png::Decoder::new(Cursor::new(png));
        let mut reader = decoder.read_info().expect("artifact is a PNG");
        let mut rgba = vec![0; reader.output_buffer_size()];
        let output = reader.next_frame(&mut rgba).expect("PNG frame decodes");
        assert_eq!((output.width, output.height), (30, 40));
        let top = pixel(&rgba, 30, 15, 14);
        let bottom = pixel(&rgba, 30, 15, 25);
        assert!(top[0] > 240 && top[2] < 20 && top[3] == 255);
        assert!(bottom[0] < 20 && bottom[2] > 240 && bottom[3] == 255);

        let pdf = String::from_utf8_lossy(&render_artifact(&scene, OutputFormat::Pdf)).into_owned();
        assert!(pdf.contains("/Width 1\n/Height 2"));
    }
}

#[test]
fn malformed_base64_charges_only_complete_valid_prefix_quanta() {
    for encoded in ["AAAA$", "TQ==$", "TWFu=AAA"] {
        let scene = image_scene(&format!("data:image/png;base64,{encoded}"));
        let limits = ResourceLimits::builder()
            .max_asset_bytes(3)
            .build()
            .expect("asset-byte limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("SVG request is valid");

        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("malformed base64 has local recovery");

        assert_eq!(
            diagnostic_codes(&outcome),
            [DiagnosticCode::ASSET_MALFORMED_FILE_RECORD],
            "encoded spelling {encoded}"
        );
    }
}

#[test]
fn direct_svg_rejects_a_png_whose_ihdr_is_not_the_first_chunk() {
    let png = fixture_png(1, 1, &[[255, 0, 0, 255]]);
    let mut malformed = Vec::from(&png[..8]);
    append_png_chunk(&mut malformed, *b"tEXt", b"key\0value");
    malformed.extend_from_slice(&png[8..]);
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(malformed)
    ));
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("invalid PNG structure has local recovery");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert_eq!(
        diagnostic_codes(&outcome),
        [DiagnosticCode::IMAGE_DECODE_FAILED]
    );
    assert!(!svg.contains("<image "));
}

#[test]
fn direct_svg_rejects_color_chunks_that_split_or_follow_image_data() {
    let source = fixture_png(1, 1, &[[255, 0, 0, 255]]);
    let idat = png_chunk_offset(&source, *b"IDAT");
    let idat_length = usize::try_from(u32::from_be_bytes(
        source[idat..idat + 4]
            .try_into()
            .expect("fixture IDAT length"),
    ))
    .expect("fixture IDAT length fits usize");
    let idat_end = idat + 12 + idat_length;
    let mut split = Vec::from(&source[..idat_end]);
    append_png_chunk(&mut split, *b"cICP", &[1, 13, 0, 1]);
    split.extend_from_slice(&source[idat..]);
    let after = png_with_chunk_before_iend(&source, *b"cICP", &[1, 13, 0, 1]);

    for malformed in [split, after] {
        let scene = image_scene(&format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(malformed)
        ));
        let outcome = render_outcome(&scene, OutputFormat::Svg);
        assert_eq!(
            diagnostic_codes(&outcome),
            [DiagnosticCode::IMAGE_DECODE_FAILED]
        );
    }
}

#[test]
fn crop_and_negative_scale_map_the_same_static_png_source_in_every_format() {
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
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    );
    let scene = cropped_image_scene(&data_url);

    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(svg.contains("<g transform=\"translate(20 20) scale(-1 -1)\">"));
    assert!(svg.contains("viewBox=\"1 0 2 2\""));
    assert!(svg.contains("width=\"3\" height=\"2\""));

    let png = render_artifact(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(&png));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut pixels).expect("PNG frame decodes");
    assert_eq!((output.width, output.height), (40, 40));
    let yellow = pixel(&pixels, 40, 14, 14);
    let magenta = pixel(&pixels, 40, 24, 14);
    let blue = pixel(&pixels, 40, 14, 24);
    let green = pixel(&pixels, 40, 24, 24);
    assert_eq!(yellow, [255, 255, 0, 255]);
    assert!(magenta[0] > 240 && magenta[1] < 20 && magenta[2] > 240 && magenta[3] == 255);
    assert!(blue[0] < 20 && blue[1] < 20 && blue[2] > 240 && blue[3] == 255);
    assert!(green[0] < 20 && green[1] > 225 && green[2] < 30 && green[3] == 255);

    let pdf_bytes = render_artifact(&scene, OutputFormat::Pdf);
    let pdf = String::from_utf8_lossy(&pdf_bytes);
    assert!(pdf.contains("-30 0 0 -20 30 20 cm\n/Im0 Do"));
}

#[test]
fn missing_local_image_uses_the_format_specific_recovery() {
    let scene = image_scene_without_files();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("missing image has a meaningful recovery");

        assert_eq!(
            diagnostic_codes(&outcome),
            [DiagnosticCode::IMAGE_MISSING_FILE]
        );
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
                assert!(!svg.contains("<image "));
                assert!(matches!(
                    outcome.report().diagnostics()[0].details(),
                    Some(ErrorDetails::Asset {
                        recovery: Recovery::Omitted,
                        recipe_id: None,
                        ..
                    })
                ));
            }
            OutputFormat::Png => {
                assert!(matches!(
                    outcome.report().diagnostics()[0].details(),
                    Some(ErrorDetails::Asset {
                        recovery: Recovery::Placeholder,
                        recipe_id: Some(recipe),
                        ..
                    }) if recipe.starts_with("official.image@sha256:")
                ));
            }
            OutputFormat::Pdf => {
                assert!(!outcome
                    .artifact()
                    .bytes()
                    .windows("/Subtype /Image".len())
                    .any(|window| window == b"/Subtype /Image"));
                assert!(matches!(
                    outcome.report().diagnostics()[0].details(),
                    Some(ErrorDetails::Asset {
                        recovery: Recovery::Omitted,
                        recipe_id: None,
                        ..
                    })
                ));
            }
            _ => panic!("revision-1 formats are covered"),
        }
    }
}

/// AVIF is scoped out of compatibility revision 1 (#68, spec 8.2): a well-formed
/// AVIF `ftyp` box is a validated-but-unaccepted media family, so it takes the
/// `image.unsupported-format` recovery of the `/dataURL` disposition in every
/// format, while the declared `image/avif` media type stays a legal declaration.
#[test]
fn avif_payload_is_not_accepted_in_revision_1_and_uses_the_unsupported_format_recovery() {
    let avif_ftyp = b"\x00\x00\x00\x1cftypavif\x00\x00\x00\x00avifmif1miaf";
    let data_url = format!(
        "data:image/avif;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(avif_ftyp)
    );
    let scene = image_scene_with_media_type_and_size(&data_url, "image/avif", 100, 80);

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("an unaccepted raster family has a local recovery");

        assert_eq!(
            diagnostic_codes(&outcome),
            [DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT]
        );
        let diagnostic = &outcome.report().diagnostics()[0];
        assert_eq!(diagnostic.json_pointer(), "/files/asset/dataURL");
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
                assert!(!svg.contains("<image "));
                assert!(matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::Asset {
                        recovery: Recovery::Omitted,
                        recipe_id: None,
                        ..
                    })
                ));
            }
            OutputFormat::Png => {
                assert!(matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::Asset {
                        recovery: Recovery::Placeholder,
                        recipe_id: Some(recipe),
                        ..
                    }) if recipe.starts_with("official.image@sha256:")
                ));
            }
            OutputFormat::Pdf => {
                assert!(!outcome
                    .artifact()
                    .bytes()
                    .windows("/Subtype /Image".len())
                    .any(|window| window == b"/Subtype /Image"));
                assert!(matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::Asset {
                        recovery: Recovery::Omitted,
                        recipe_id: None,
                        ..
                    })
                ));
            }
            _ => panic!("revision-1 formats are covered"),
        }
    }
}

#[test]
fn png_missing_image_paints_the_selected_official_placeholder() {
    let scene = SceneDocument::parse(
        br#"{
            "type":"excalidraw","version":2,
            "elements":[{
                "id":"image","type":"image","x":0,"y":0,"width":100,"height":80,
                "angle":0,"opacity":100,"fileId":"missing","status":"saved","scale":[1,1],
                "seed":1,"updated":1
            }],
            "appState":{},"files":{}
        }"#,
    )
    .expect("missing-image scene is admitted");
    let png = render_artifact(&scene, OutputFormat::Png);
    let decoder = png::Decoder::new(Cursor::new(&png));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let output = reader.next_frame(&mut pixels).expect("PNG frame decodes");

    assert_eq!((output.width, output.height), (120, 100));
    assert_eq!(pixel(&pixels, 120, 15, 15), [231, 231, 231, 255]);
    let icon = pixel(&pixels, 120, 60, 50);
    assert!(icon[0] < 231 && icon[0] == icon[1] && icon[1] == icon[2] && icon[3] == 255);
}

fn image_scene(data_url: &str) -> SceneDocument {
    image_scene_with_media_type(data_url, "image/png")
}

fn image_scene_with_media_type(data_url: &str, media_type: &str) -> SceneDocument {
    image_scene_with_media_type_and_size(data_url, media_type, 1, 1)
}

fn image_scene_with_size(data_url: &str, width: u32, height: u32) -> SceneDocument {
    image_scene_with_media_type_and_size(data_url, "image/png", width, height)
}

fn image_scene_with_media_type_and_size(
    data_url: &str,
    media_type: &str,
    width: u32,
    height: u32,
) -> SceneDocument {
    let json = format!(
        r#"{{
            "type":"excalidraw",
            "version":2,
            "elements":[{{
                "id":"image",
                "type":"image",
                "x":0,
                "y":0,
                "width":{width},
                "height":{height},
                "angle":0,
                "opacity":100,
                "fileId":"asset",
                "status":"saved",
                "scale":[1,1],
                "seed":1,
                "updated":1
            }}],
            "appState":{{}},
            "files":{{
                "asset":{{
                    "id":"asset",
                    "mimeType":"{media_type}",
                    "dataURL":"{data_url}",
                    "created":1
                }}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}

fn image_scene_without_files() -> SceneDocument {
    SceneDocument::parse(
        br#"{
            "type":"excalidraw","version":2,
            "elements":[{
                "id":"image","type":"image","x":0,"y":0,"width":1,"height":1,
                "angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],
                "seed":1,"updated":1
            }],
            "appState":{},"files":{}
        }"#,
    )
    .expect("supplement-only scene is admitted")
}

fn two_image_scene() -> SceneDocument {
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"one","type":"image","x":0,"y":0,"width":1,"height":1,"angle":0,"opacity":100,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
                {{"id":"two","type":"image","x":2,"y":0,"width":1,"height":1,"angle":0,"opacity":100,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
            ],
            "appState":{{}},
            "files":{{
                "one":{{"id":"one","mimeType":"image/png","dataURL":"data:image/png;base64,{PNG_BASE64}","created":1}},
                "two":{{"id":"two","mimeType":"image/png","dataURL":"data:image/png;base64,{PNG_BASE64}","created":1}}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("duplicate-payload scene is admitted")
}

fn two_payload_scene(first: &[u8], second: &[u8]) -> SceneDocument {
    let first = base64::engine::general_purpose::STANDARD.encode(first);
    let second = base64::engine::general_purpose::STANDARD.encode(second);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"one","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
                {{"id":"two","type":"image","x":2,"y":0,"width":1,"height":1,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
            ],
            "appState":{{}},"files":{{
                "one":{{"id":"one","mimeType":"image/png","dataURL":"data:image/png;base64,{first}","created":1}},
                "two":{{"id":"two","mimeType":"image/png","dataURL":"data:image/png;base64,{second}","created":1}}
            }}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("two-payload scene is admitted")
}

fn diagnostic_codes(outcome: &excalidraw_render::RenderOutcome) -> Vec<DiagnosticCode> {
    outcome
        .report()
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code())
        .collect()
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

fn fixture_apng(first: &[[u8; 4]], second: &[[u8; 4]]) -> Vec<u8> {
    assert_eq!(first.len(), second.len());
    let width = u32::try_from(first.len()).expect("fixture width fits u32");
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .set_animated(2, 0)
            .expect("fixture animation is valid");
        let mut writer = encoder.write_header().expect("fixture APNG header encodes");
        let first: Vec<u8> = first.iter().flatten().copied().collect();
        let second: Vec<u8> = second.iter().flatten().copied().collect();
        writer
            .write_image_data(&first)
            .expect("first APNG frame encodes");
        writer
            .write_image_data(&second)
            .expect("second APNG frame encodes");
    }
    bytes
}

fn apng_with_separate_default(corrupt_animation_frame: bool) -> Vec<u8> {
    let source = fixture_apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let mut output = Vec::from(&source[..8]);
    let mut offset = 8;
    let mut frame_control_count = 0;
    while offset < source.len() {
        let length = usize::try_from(u32::from_be_bytes(
            source[offset..offset + 4]
                .try_into()
                .expect("fixture chunk length"),
        ))
        .expect("fixture chunk length fits usize");
        let chunk_type: [u8; 4] = source[offset + 4..offset + 8]
            .try_into()
            .expect("fixture chunk type");
        let mut data = source[offset + 8..offset + 8 + length].to_vec();
        match &chunk_type {
            b"acTL" => data[..4].copy_from_slice(&1_u32.to_be_bytes()),
            b"fcTL" => {
                frame_control_count += 1;
                if frame_control_count == 1 {
                    offset += 12 + length;
                    continue;
                }
                data[..4].copy_from_slice(&0_u32.to_be_bytes());
            }
            b"fdAT" => {
                data[..4].copy_from_slice(&1_u32.to_be_bytes());
                if corrupt_animation_frame {
                    data[4] ^= 0xff;
                }
            }
            _ => {}
        }
        append_png_chunk(&mut output, chunk_type, &data);
        offset += 12 + length;
    }
    output
}

fn apng_frame_control(sequence: u32, width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(26);
    data.extend_from_slice(&sequence.to_be_bytes());
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&0_u32.to_be_bytes());
    data.extend_from_slice(&0_u32.to_be_bytes());
    data.extend_from_slice(&1_u16.to_be_bytes());
    data.extend_from_slice(&30_u16.to_be_bytes());
    data.extend_from_slice(&[0, 0]);
    data
}

fn exif_orientation(orientation: u16) -> Vec<u8> {
    let mut exif = Vec::new();
    exif.extend_from_slice(b"II");
    exif.extend_from_slice(&42_u16.to_le_bytes());
    exif.extend_from_slice(&8_u32.to_le_bytes());
    exif.extend_from_slice(&1_u16.to_le_bytes());
    exif.extend_from_slice(&0x0112_u16.to_le_bytes());
    exif.extend_from_slice(&3_u16.to_le_bytes());
    exif.extend_from_slice(&1_u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&0_u16.to_le_bytes());
    exif.extend_from_slice(&0_u32.to_le_bytes());
    exif
}

fn iso_gain_map_metadata() -> Vec<u8> {
    let mut metadata = Vec::new();
    metadata.extend_from_slice(&0_u16.to_be_bytes());
    metadata.extend_from_slice(&0_u16.to_be_bytes());
    metadata.push(0x40);
    for (numerator, denominator) in [
        (0_i32, 1_u32),
        (1, 1),
        (0, 1),
        (1, 1),
        (1, 1),
        (0, 1),
        (0, 1),
    ] {
        metadata.extend_from_slice(&numerator.to_be_bytes());
        metadata.extend_from_slice(&denominator.to_be_bytes());
    }
    metadata
}

fn append_png_chunk(bytes: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) {
    bytes.extend_from_slice(
        &u32::try_from(data.len())
            .expect("fixture chunk length fits u32")
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&chunk_type);
    bytes.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(&chunk_type);
    crc.update(data);
    bytes.extend_from_slice(&crc.finalize().to_be_bytes());
}

fn png_with_chunks_before_idat(source: &[u8], chunks: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let idat = png_chunk_offset(source, *b"IDAT");
    let mut png = Vec::from(&source[..idat]);
    for (chunk_type, data) in chunks {
        append_png_chunk(&mut png, *chunk_type, data);
    }
    png.extend_from_slice(&source[idat..]);
    png
}

fn png_with_chunk_before_iend(source: &[u8], chunk_type: [u8; 4], data: &[u8]) -> Vec<u8> {
    let iend = png_chunk_offset(source, *b"IEND");
    let mut png = Vec::from(&source[..iend]);
    append_png_chunk(&mut png, chunk_type, data);
    png.extend_from_slice(&source[iend..]);
    png
}

fn mutate_first_idat(png: &mut [u8], mutate: impl FnOnce(&mut [u8])) {
    mutate_first_png_chunk(png, *b"IDAT", mutate);
}

fn mutate_first_png_chunk(png: &mut [u8], chunk_type: [u8; 4], mutate: impl FnOnce(&mut [u8])) {
    let offset = png_chunk_offset(png, chunk_type);
    let length = u32::from_be_bytes(png[offset..offset + 4].try_into().expect("chunk length"));
    let length = usize::try_from(length).expect("chunk length fits usize");
    let data_start = offset + 8;
    let data_end = data_start + length;
    mutate(&mut png[data_start..data_end]);
    let mut crc = crc32fast::Hasher::new();
    crc.update(&chunk_type);
    crc.update(&png[data_start..data_end]);
    png[data_end..data_end + 4].copy_from_slice(&crc.finalize().to_be_bytes());
}

fn png_chunk_offset(png: &[u8], wanted: [u8; 4]) -> usize {
    let mut offset = 8;
    loop {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().expect("chunk length"));
        if png[offset + 4..offset + 8] == wanted {
            return offset;
        }
        offset += 12 + usize::try_from(length).expect("chunk length fits usize");
    }
}

fn rendered_image_center(source: &[u8]) -> [u8; 4] {
    rendered_image_center_with_background(source, true)
}

fn rendered_image_center_without_background(source: &[u8]) -> [u8; 4] {
    rendered_image_center_with_background(source, false)
}

fn rendered_image_center_with_background(source: &[u8], include_background: bool) -> [u8; 4] {
    let scene = image_scene(&format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(source)
    ));
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(include_background)
        .build()
        .expect("PNG request is valid");
    let png = Renderer::new()
        .render(&scene, &request)
        .expect("PNG-family asset renders")
        .artifact()
        .bytes()
        .to_vec();
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("artifact is a PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut rgba).expect("PNG frame decodes");
    pixel(&rgba, 21, 10, 10)
}

fn two_supplemental_image_scene(first: &str, second: &str) -> SceneDocument {
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"first","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"{first}","status":"saved","scale":[1,1],"seed":1,"updated":1}},
                {{"id":"second","type":"image","x":2,"y":0,"width":1,"height":1,"fileId":"{second}","status":"saved","scale":[1,1],"seed":2,"updated":1}}
            ],
            "appState":{{}},"files":{{}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("two-image scene is admitted")
}

fn cropped_image_scene(data_url: &str) -> SceneDocument {
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[{{
                "id":"image","type":"image","x":0,"y":0,"width":20,"height":20,
                "angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[-1,-1],
                "crop":{{"x":1,"y":0,"width":2,"height":2,"naturalWidth":3,"naturalHeight":2}},
                "seed":1,"updated":1
            }}],
            "appState":{{}},"files":{{"asset":{{
                "id":"asset","mimeType":"image/png","dataURL":"{data_url}","created":1
            }}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("cropped image scene is admitted")
}

fn render_artifact(scene: &SceneDocument, format: OutputFormat) -> Vec<u8> {
    render_outcome(scene, format).artifact().bytes().to_vec()
}

fn render_outcome(scene: &SceneDocument, format: OutputFormat) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .build()
        .expect("request is valid");
    Renderer::new()
        .render(scene, &request)
        .expect("PNG-family asset renders")
}

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    pixels[(y * width + x) * 4..(y * width + x + 1) * 4]
        .try_into()
        .expect("pixel has four channels")
}
