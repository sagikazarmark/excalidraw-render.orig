//! Static PNG and APNG assets end to end: the octets a direct SVG passes
//! through, the pixels the PNG target projects, the lossless PDF embedding,
//! the session's asset diagnostics (media claims, supplements, animation
//! flattening, missing and unsupported payloads) and every accounting test.
//! Decoder behaviour (colour chunks, orientation, APNG frame validation, the
//! malformed chunk layouts) is tested at the asset cache's own seam in
//! `src/render/asset/tests.rs`.

use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorCode, ErrorDetails, OutputFormat, Recovery, RenderRequest, Renderer,
    ResourceLimits, SceneDocument, SupplementalAssets,
};

use crate::common::codec_fixtures::{
    apng, exif_orientation_tiff, png_rgba, png_with_chunks_before_idat,
};
use crate::common::image_scene::{
    data_url, image_scene, image_scene_from_data_url, image_scene_with_element_fields,
};
use crate::common::pdf_inspect::pdf_text;
use crate::common::raster::decode_png;
use crate::common::render::{render, render_with_request};
use crate::common::svg::{image_symbol, image_use};

const PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

#[test]
fn direct_svg_preserves_validated_static_png_octets_in_a_canonical_data_url() {
    let scene = fixture_png_scene();

    let outcome = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    // The octets live once in the shared definition (spec 8.4); the element
    // draws them through a `<use>` sized to its viewport.
    assert!(svg.contains(&image_symbol(
        0,
        &format!("data:image/png;base64,{PNG_BASE64}")
    )));
    assert!(svg.contains(&image_use(0, "1", "1", "")));
}

#[test]
fn png_projects_canonical_rgba_pixels_into_the_image_occurrence() {
    let scene = fixture_png_scene();

    let outcome = render(&scene, OutputFormat::Png);
    let decoded = decode_png(outcome.artifact().bytes());

    assert_eq!((decoded.width, decoded.height), (21, 21));
    assert_eq!(decoded.pixel(10, 10), [0, 0, 0, 255]);
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn pdf_embeds_static_png_as_a_lossless_rgb_image_with_a_soft_mask() {
    let scene = fixture_png_scene();

    let outcome = render(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(outcome.artifact().bytes());

    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/ColorSpace [/ICCBased 5 0 R]"));
    assert!(pdf.contains("/ColorSpace /DeviceGray"));
    assert!(pdf.contains("/SMask "));
    assert!(pdf.contains("/Im0 Do"));
    assert!(!pdf.contains("pdf.raster-fallback"));
    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn apng_is_preserved_in_svg_and_flattened_to_frame_zero_in_png_and_pdf() {
    let apng = apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let scene = image_scene(&apng, "image/png", 1, 1);

    let svg_outcome = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(&data_url("image/png", &apng)));
    assert!(svg_outcome.report().diagnostics().is_empty());

    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format);
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
            assert_eq!(
                decode_png(outcome.artifact().bytes()).pixel(10, 10),
                [255, 0, 0, 255]
            );
        }
    }
}

#[test]
fn shared_apng_bytes_report_each_rendered_occurrence() {
    let apng = apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let scene = two_payload_scene(&apng, &apng);

    let outcome = render(&scene, OutputFormat::Png);

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
    let apng = apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let payload_bytes = u64::try_from(apng.len()).expect("fixture length fits u64");
    let scene = image_scene(&apng, "image/png", 1, 1);

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
        let outcome = render_with_request(&scene, &request);

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
    let scene = fixture_png_scene();
    for (limit, expected_diagnostics) in [(32, 0), (31, 1)] {
        let limits = ResourceLimits::builder()
            .max_element_work_units(limit)
            .build()
            .expect("element-work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Png)
            .resource_limits(limits)
            .build()
            .expect("PNG request is valid");
        let outcome = render_with_request(&scene, &request);

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
    let scene = fixture_png_scene();
    for (limit, expected_diagnostics) in [(32, 0), (31, 1)] {
        let limits = ResourceLimits::builder()
            .max_element_work_units(limit)
            .build()
            .expect("element-work limit is valid");
        let request = RenderRequest::builder(OutputFormat::Png)
            .resource_limits(limits)
            .build()
            .expect("PNG request is valid");
        let outcome = render_with_request(&scene, &request);

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

    let outcome = render_with_request(&scene, &request);
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
    let scene = fixture_png_scene();
    let assets = SupplementalAssets::builder()
        .asset("asset", b"not the scene PNG".to_vec())
        .build()
        .expect("supplemental mapping is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("SVG request is valid");

    let outcome = render_with_request(&scene, &request);
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

    let outcome = render_with_request(&scene, &request);
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

    let outcome = render_with_request(&scene, &request);
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

        let outcome = render_with_request(&image_scene_without_files(), &request);

        assert_eq!(diagnostic_codes(&outcome), [expected]);
        assert_eq!(outcome.report().diagnostics()[0].json_pointer(), "");
    }
}

#[test]
fn sniffed_png_type_wins_over_a_scene_media_claim() {
    let scene = image_scene_from_data_url(
        &format!("data:image/png;base64,{PNG_BASE64}"),
        "image/jpeg",
        1,
        1,
    );

    let outcome = render(&scene, OutputFormat::Svg);
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

    let outcome = render(&scene, OutputFormat::Svg);

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

    let outcome = render(&scene, OutputFormat::Svg);

    assert!(outcome.report().diagnostics().is_empty());
}

#[test]
fn duplicate_semantic_png_bytes_share_one_pdf_resource_but_render_twice() {
    let scene = two_image_scene();

    let outcome = render(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(outcome.artifact().bytes());

    assert_eq!(pdf.matches("/ColorSpace [/ICCBased 5 0 R]").count(), 1);
    assert_eq!(pdf.matches("/Im0 Do").count(), 2);
    assert!(outcome.report().diagnostics().is_empty());
}

/// Direct SVG shares an asset the way the reference's `reuseImages` export
/// does (spec 8.4, #151): one `<symbol>` per semantic asset identity in
/// `<defs>`, keyed by the source index of the first element drawing it, and
/// one `<use>` per element. Two file records with the same bytes are one
/// identity; two payloads are two, in first-use order.
#[test]
fn duplicate_semantic_png_bytes_share_one_svg_symbol_and_distinct_payloads_do_not() {
    let shared = render(&two_image_scene(), OutputFormat::Svg);
    let svg = std::str::from_utf8(shared.artifact().bytes()).expect("SVG is UTF-8");
    assert!(shared.report().diagnostics().is_empty());
    assert_eq!(svg.matches("<symbol ").count(), 1);
    assert_eq!(svg.matches("<image ").count(), 1);
    assert!(svg.contains(&image_symbol(
        0,
        &format!("data:image/png;base64,{PNG_BASE64}")
    )));
    assert_eq!(svg.matches(&image_use(0, "1", "1", "")).count(), 2);

    let red = png_rgba(1, 1, &[[255, 0, 0, 255]]);
    let blue = png_rgba(1, 1, &[[0, 0, 255, 255]]);
    let distinct = render(&two_payload_scene(&red, &blue), OutputFormat::Svg);
    let svg = std::str::from_utf8(distinct.artifact().bytes()).expect("SVG is UTF-8");
    let defs = &svg[svg.find("<defs>").expect("SVG has <defs>")..];
    let defs = &defs[..defs.find("</defs>").expect("SVG closes <defs>")];
    assert_eq!(
        defs,
        format!(
            "<defs>{}{}",
            image_symbol(0, &data_url("image/png", &red)),
            image_symbol(1, &data_url("image/png", &blue))
        )
    );
    assert_eq!(svg.matches(&image_use(0, "1", "1", "")).count(), 1);
    assert_eq!(svg.matches(&image_use(1, "1", "1", "")).count(), 1);
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
    let red = png_rgba(1, 1, &[[255, 0, 0, 255]]);
    let blue = png_rgba(1, 1, &[[0, 0, 255, 255]]);
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
    let png = png_rgba(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let scene = image_scene(&png, "image/png", 1, 1);
    let limits = ResourceLimits::builder()
        .max_asset_bytes(u64::try_from(png.len() - 1).expect("fixture length fits u64"))
        .build()
        .expect("asset-byte limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("SVG request is valid");

    let outcome = render_with_request(&scene, &request);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert_eq!(
        diagnostic_codes(&outcome),
        [DiagnosticCode::RESOURCE_ASSET_BYTES_EXCEEDED]
    );
    assert!(!svg.contains("<image "));
}

#[test]
fn png_dimension_pixel_and_work_limits_recover_from_ihdr_before_decode() {
    let png = png_rgba(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let scene = image_scene(&png, "image/png", 1, 1);
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
        let outcome = render_with_request(&scene, &request);
        assert_eq!(diagnostic_codes(&outcome), [expected]);
    }
}

#[test]
fn direct_svg_validates_static_png_without_deriving_decoded_pixels() {
    let png = png_rgba(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]);
    let scene = image_scene(&png, "image/png", 1, 1);
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

    let outcome = render_with_request(&scene, &request);
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(outcome.report().diagnostics().is_empty());
    assert!(svg.contains("<image "));
}

/// The one cross-format orientation check: the asset cache turns the pixels
/// and swaps the dimensions (tested at its seam for every orientation and
/// codec), and each encoder then consumes the oriented resolution while the
/// direct SVG keeps the source bytes.
#[test]
fn png_and_pdf_apply_png_exif_orientation_while_svg_preserves_source_bytes() {
    let oriented = png_with_chunks_before_idat(
        &png_rgba(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]]),
        &[(*b"eXIf", exif_orientation_tiff(6))],
    );
    let scene = image_scene(&oriented, "image/png", 10, 20);

    let svg_outcome = render(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(&data_url("image/png", &oriented)));

    let decoded = decode_png(render(&scene, OutputFormat::Png).artifact().bytes());
    assert_eq!((decoded.width, decoded.height), (30, 40));
    let top = decoded.pixel(15, 14);
    let bottom = decoded.pixel(15, 25);
    assert!(top[0] > 240 && top[2] < 20 && top[3] == 255);
    assert!(bottom[0] < 20 && bottom[2] > 240 && bottom[3] == 255);

    let pdf = render(&scene, OutputFormat::Pdf);
    let pdf = pdf_text(pdf.artifact().bytes());
    assert!(pdf.contains("/Width 1\n/Height 2"));
}

#[test]
fn malformed_base64_charges_only_complete_valid_prefix_quanta() {
    for encoded in ["AAAA$", "TQ==$", "TWFu=AAA"] {
        let scene = png_scene(&format!("data:image/png;base64,{encoded}"));
        let limits = ResourceLimits::builder()
            .max_asset_bytes(3)
            .build()
            .expect("asset-byte limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("SVG request is valid");

        let outcome = render_with_request(&scene, &request);

        assert_eq!(
            diagnostic_codes(&outcome),
            [DiagnosticCode::ASSET_MALFORMED_FILE_RECORD],
            "encoded spelling {encoded}"
        );
    }
}

#[test]
fn crop_and_negative_scale_map_the_same_static_png_source_in_every_format() {
    let png = png_rgba(
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
    let scene = image_scene_with_element_fields(
        Some(("image/png", &png)),
        r#""x":0,"y":0,"width":20,"height":20,"angle":0,"opacity":100,"scale":[-1,-1],
           "crop":{"x":1,"y":0,"width":2,"height":2,"naturalWidth":3,"naturalHeight":2}"#,
    );

    let svg = render(&scene, OutputFormat::Svg)
        .artifact()
        .bytes()
        .to_vec();
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    assert!(svg.contains("<g transform=\"translate(20 20) scale(-1 -1)\">"));
    // The whole 3x2 source at its uncropped 30x20 under the 10 px crop offset,
    // through the reference's viewport from that offset to the far edges.
    assert!(svg.contains("<svg x=\"-10\" y=\"0\" width=\"30\" height=\"20\" overflow=\"hidden\">"));
    assert!(svg.contains(&image_use(0, "30", "20", "")));

    let decoded = decode_png(render(&scene, OutputFormat::Png).artifact().bytes());
    assert_eq!((decoded.width, decoded.height), (40, 40));
    let yellow = decoded.pixel(14, 14);
    let magenta = decoded.pixel(24, 14);
    let blue = decoded.pixel(14, 24);
    let green = decoded.pixel(24, 24);
    assert_eq!(yellow, [255, 255, 0, 255]);
    assert!(magenta[0] > 240 && magenta[1] < 20 && magenta[2] > 240 && magenta[3] == 255);
    assert!(blue[0] < 20 && blue[1] < 20 && blue[2] > 240 && blue[3] == 255);
    assert!(green[0] < 20 && green[1] > 225 && green[2] < 30 && green[3] == 255);

    let pdf_bytes = render(&scene, OutputFormat::Pdf)
        .artifact()
        .bytes()
        .to_vec();
    let pdf = pdf_text(&pdf_bytes);
    assert!(pdf.contains("-30 0 0 20 30 0 cm\n/Im0 Do"));
}

#[test]
fn missing_local_image_uses_the_format_specific_recovery() {
    let scene = image_scene_without_files();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format);

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
    let scene = image_scene(avif_ftyp, "image/avif", 100, 80);

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format);

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
    let scene = image_scene_with_element_fields(
        None,
        r#""x":0,"y":0,"width":100,"height":80,"angle":0,"opacity":100,"scale":[1,1]"#,
    );
    let decoded = decode_png(render(&scene, OutputFormat::Png).artifact().bytes());

    assert_eq!((decoded.width, decoded.height), (120, 100));
    assert_eq!(decoded.pixel(15, 15), [231, 231, 231, 255]);
    let icon = decoded.pixel(60, 50);
    assert!(icon[0] < 231 && icon[0] == icon[1] && icon[1] == icon[2] && icon[3] == 255);
}

/// A one-pixel image over a PNG-typed record carrying `data_url` as written.
fn png_scene(data_url: &str) -> SceneDocument {
    image_scene_from_data_url(data_url, "image/png", 1, 1)
}

/// [`png_scene`] over the one-pixel black `PNG_BASE64` fixture.
fn fixture_png_scene() -> SceneDocument {
    png_scene(&format!("data:image/png;base64,{PNG_BASE64}"))
}

/// A one-pixel image whose `asset` file is absent from the scene.
fn image_scene_without_files() -> SceneDocument {
    image_scene_with_element_fields(
        None,
        r#""x":0,"y":0,"width":1,"height":1,"angle":0,"opacity":100,"scale":[1,1]"#,
    )
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
    let first = data_url("image/png", first);
    let second = data_url("image/png", second);
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"one","type":"image","x":0,"y":0,"width":1,"height":1,"fileId":"one","status":"saved","scale":[1,1],"seed":1,"updated":1}},
                {{"id":"two","type":"image","x":2,"y":0,"width":1,"height":1,"fileId":"two","status":"saved","scale":[1,1],"seed":2,"updated":1}}
            ],
            "appState":{{}},"files":{{
                "one":{{"id":"one","mimeType":"image/png","dataURL":"{first}","created":1}},
                "two":{{"id":"two","mimeType":"image/png","dataURL":"{second}","created":1}}
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
