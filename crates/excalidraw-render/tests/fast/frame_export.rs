use excalidraw_render::{
    ColorMode, CompatibilityRevision, ErrorCode, ErrorFamily, FrameBatchErrorKind, FrameKind,
    FrameRenderRequest, MetadataOptions, OutputFormat, PdfOptions, PdfRasterFallback, Renderer,
    ResourceLimits, SceneDocument, Sizing, SupplementalAssets,
};
use sha2::{Digest, Sha256};

use crate::common::raster::decode_png;
use crate::common::repository_root;

const FRAME_SCENE: &[u8] = br#"{
    "type":"excalidraw","version":2,
    "elements":[
        null,
        {"id":"ordinary","type":"frame","name":"Roadmap","x":0,"y":0,"width":100,"height":80},
        {"id":"deleted","type":"frame","name":"Old","isDeleted":true},
        {"id":"magic","type":"magicframe","name":null,"x":120,"y":0,"width":60,"height":40},
        {"id":"empty-name","type":"frame","name":"","x":0,"y":100,"width":20,"height":20}
    ],
    "appState":{},"files":{}
}"#;

#[test]
fn live_frame_occurrences_are_discovered_in_source_order() {
    let scene = SceneDocument::parse(FRAME_SCENE).expect("frame scene is admitted");
    let frames = scene.frames().collect::<Vec<_>>();

    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].ordinal(), 1);
    assert_eq!(frames[0].kind(), FrameKind::Frame);
    assert_eq!(frames[0].name(), Some("Roadmap"));
    assert_eq!(frames[1].ordinal(), 2);
    assert_eq!(frames[1].kind(), FrameKind::MagicFrame);
    assert_eq!(frames[1].name(), None);
    assert_eq!(frames[2].ordinal(), 3);
    assert_eq!(frames[2].kind(), FrameKind::Frame);
    assert_eq!(frames[2].name(), Some(""));
}

#[test]
fn frame_artifacts_use_the_frame_profile_and_schema_2_target_manifest() {
    let scene = SceneDocument::parse(FRAME_SCENE).expect("frame scene is admitted");
    let frame = scene.frames().next().expect("named frame exists");
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .metadata_options(MetadataOptions::new(true, true, true))
        .build()
        .expect("frame metadata request is valid");
    let renderer = Renderer::new();

    let frame_profile = Renderer::frame_profile_identity(CompatibilityRevision::REVISION_1)
        .expect("frame profile exists");
    let profile_path =
        repository_root().join("tests/oracle/reference-profiles/revision-1-frame.json");
    let profile_bytes = std::fs::read(profile_path).expect("frame profile is checked");
    assert_eq!(
        frame_profile.profile_digest(),
        format!("sha256:{:x}", Sha256::digest(profile_bytes))
    );
    assert_eq!(
        renderer.selected_frame_profile_identity().profile_digest(),
        frame_profile.profile_digest()
    );
    assert_ne!(
        frame_profile.profile_digest(),
        renderer.selected_profile_identity().profile_digest()
    );

    let outcome = renderer
        .render_frame(frame, &request)
        .expect("frame with metadata renders");
    assert_eq!(outcome.profile_digest(), frame_profile.profile_digest());
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains("urn:excalidraw-render:artifact-manifest:1"));
    assert!(svg.contains("{\"schema_version\":2,"));
    assert!(svg.contains("\"target\":{\"kind\":\"frame\",\"ordinal\":1,\"name\":\"Roadmap\"}"));
    assert!(svg.contains(frame_profile.profile_digest()));
    assert!(svg.contains("\"embedded_scene\":{"));

    let png = renderer
        .render_frame(
            frame,
            &FrameRenderRequest::builder(OutputFormat::Png)
                .metadata_options(MetadataOptions::new(true, true, true))
                .build()
                .expect("PNG frame metadata request is valid"),
        )
        .expect("frame with metadata renders to PNG");
    assert_eq!(png.profile_digest(), frame_profile.profile_digest());
    assert!(contains_bytes(
        png.artifact().bytes(),
        b"\"schema_version\":2,"
    ));
    assert!(contains_bytes(
        png.artifact().bytes(),
        b"\"kind\":\"frame\",\"ordinal\":1,\"name\""
    ));

    let pdf = renderer
        .render_frame(
            frame,
            &FrameRenderRequest::builder(OutputFormat::Pdf)
                .metadata_options(MetadataOptions::new(true, true, true))
                .build()
                .expect("PDF frame metadata request is valid"),
        )
        .expect("frame with metadata renders to PDF");
    assert_eq!(pdf.profile_digest(), frame_profile.profile_digest());
    let pdf = String::from_utf8_lossy(pdf.artifact().bytes());
    assert!(pdf.contains("<er:manifest>{\"schema_version\":2,"));
    assert!(pdf.contains("\"target\":{\"kind\":\"frame\",\"ordinal\":1,\"name\":\"Roadmap\"}"));

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let metadata_off = FrameRenderRequest::builder(format)
            .build()
            .expect("metadata-off request is valid");
        let metadata_off = renderer
            .render_frame(frame, &metadata_off)
            .expect("metadata-off frame renders");
        assert!(!metadata_off
            .artifact()
            .bytes()
            .windows(b"excalidraw-render:manifest".len())
            .any(|window| window == b"excalidraw-render:manifest"));
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn every_frame_format_uses_the_target_dimensions_independently() {
    let scene = SceneDocument::parse(FRAME_SCENE).expect("frame scene is admitted");
    let frame = scene.frames().next().expect("frame exists");
    let renderer = Renderer::new();

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = FrameRenderRequest::builder(format)
            .build()
            .expect("frame request is valid");
        let outcome = renderer
            .render_frame(frame, &request)
            .expect("empty frame remains meaningful");
        match format {
            OutputFormat::Svg => assert!(std::str::from_utf8(outcome.artifact().bytes())
                .expect("SVG is UTF-8")
                .contains("viewBox=\"0 0 100 80\"")),
            OutputFormat::Png => {
                let decoded = decode_png(outcome.artifact().bytes());
                assert_eq!((decoded.width, decoded.height), (100, 80));
            }
            OutputFormat::Pdf => {
                let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
                assert!(pdf.contains("/MediaBox [0 0 75 60]"), "{pdf}");
            }
            _ => unreachable!("the test enumerates every supported frame format"),
        }
    }
}

#[test]
fn lazy_frame_rendering_validates_and_canonicalizes_targets() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"first","type":"frame","x":0,"y":0,"width":40,"height":30,"updated":1},
            {"id":"first-child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"first","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
            {"id":"second","type":"magicframe","x":100,"y":0,"width":50,"height":20,"updated":1},
            {"id":"second-child","type":"ellipse","x":110,"y":5,"width":10,"height":10,"frameId":"second","strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("frame scene is admitted");
    let frames = scene.frames().collect::<Vec<_>>();
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("frame request is valid");
    let renderer = Renderer::new();

    let expected = renderer
        .render_frame(frames[0], &request)
        .expect("single frame renders");
    let mut cursor = renderer
        .render_frames(&scene, [frames[1], frames[0]], &request)
        .expect("selected frames are valid");
    let first = cursor.next().expect("first result exists");
    assert_eq!(first.frame().ordinal(), 1);
    assert_eq!(
        first
            .result()
            .as_ref()
            .expect("first frame renders")
            .artifact()
            .bytes(),
        expected.artifact().bytes()
    );
    let second = cursor.next().expect("second result exists");
    assert_eq!(second.frame().ordinal(), 2);
    assert!(second.result().is_ok());
    assert!(cursor.next().is_none());

    let duplicate = renderer
        .render_frames(&scene, [frames[0], frames[0]], &request)
        .expect_err("duplicate targets are rejected");
    assert_eq!(duplicate.kind(), FrameBatchErrorKind::DuplicateTarget);
    assert_eq!(duplicate.ordinal(), Some(1));
    assert_eq!(duplicate.descriptor().family(), ErrorFamily::FrameBatch);
    assert!(duplicate.descriptor().details().is_none());
    assert_eq!(
        duplicate.descriptor().code(),
        ErrorCode::FRAME_BATCH_DUPLICATE_TARGET
    );

    let other_scene = SceneDocument::parse(FRAME_SCENE).expect("other scene is admitted");
    let foreign = other_scene.frames().next().expect("foreign frame exists");
    let foreign = renderer
        .render_frames(&scene, [foreign], &request)
        .expect_err("foreign targets are rejected");
    assert_eq!(foreign.kind(), FrameBatchErrorKind::ForeignTarget);
    assert_eq!(foreign.ordinal(), Some(1));
    assert_eq!(
        foreign.descriptor().code(),
        ErrorCode::FRAME_BATCH_FOREIGN_TARGET
    );

    let mut empty = renderer
        .render_frames(&scene, [], &request)
        .expect("an empty library selection is valid");
    assert!(empty.next().is_none());
    assert!(empty.global_diagnostics().is_empty());
}

#[test]
fn lazy_frame_rendering_attributes_only_applicable_diagnostics() {
    let scene = SceneDocument::parse(
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"target","type":"frame","x":0,"y":0,"width":40,"height":30},
                {"id":"child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"target","strokeColor":"not-a-color","roughness":0},
                {"id":"outside","type":"rectangle","x":100,"y":100,"width":10,"height":10,"strokeColor":"also-invalid","roughness":0}
            ],
            "appState":{"viewBackgroundColor":"invalid-background"},"files":{}
        }"##,
    )
    .expect("diagnostic frame scene is admitted");
    let frame = scene.frames().next().expect("target frame exists");
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("frame request is valid");
    let mut cursor = Renderer::new()
        .render_frames(&scene, [frame], &request)
        .expect("frame selection is valid");

    let entry = cursor.next().expect("frame result exists");
    let outcome = entry.result().as_ref().expect("frame renders");
    assert!(cursor
        .global_diagnostics()
        .iter()
        .any(|diagnostic| { diagnostic.json_pointer() == "/appState/viewBackgroundColor" }));
    assert!(entry
        .target_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.json_pointer().starts_with("/elements/0")));
    assert!(entry
        .target_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.json_pointer().starts_with("/elements/1")));
    assert!(outcome
        .report()
        .diagnostics()
        .iter()
        .all(|diagnostic| !diagnostic.json_pointer().starts_with("/elements/2")));
}

#[test]
fn unrelated_diagnostics_do_not_consume_a_frame_diagnostic_limit() {
    let scene = SceneDocument::parse(
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"outside-one","type":"rectangle","x":100,"y":100,"width":10,"height":10,"strokeColor":"invalid-one","roughness":0,"updated":1},
                {"id":"outside-two","type":"rectangle","x":120,"y":100,"width":10,"height":10,"strokeColor":"invalid-two","roughness":0,"updated":1},
                {"id":"target","type":"frame","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"target","strokeColor":"invalid-target","roughness":0,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("diagnostic-limit scene is admitted");
    let frame = scene.frames().next().expect("target frame exists");
    let limits = ResourceLimits::builder()
        .max_diagnostics(1)
        .build()
        .expect("diagnostic limit is valid");
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("frame request is valid");

    let outcome = Renderer::new()
        .render_frame(frame, &request)
        .expect("only the participating diagnostic consumes the limit");
    assert_eq!(outcome.report().diagnostics().len(), 1);
    assert_eq!(
        outcome.report().diagnostics()[0].json_pointer(),
        "/elements/3/strokeColor"
    );
}

#[test]
fn frame_export_uses_reference_crop_and_participation() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"target","type":"frame","name":"Target","x":10,"y":20,"width":100,"height":80,"roughness":0,"updated":1},
            {"id":"member","type":"rectangle","x":20,"y":30,"width":20,"height":20,"frameId":"target","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
            {"id":"unframed","type":"rectangle","x":50,"y":40,"width":20,"height":20,"strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1},
            {"id":"other","type":"frame","x":10,"y":20,"width":100,"height":80,"roughness":0,"updated":1},
            {"id":"other-child","type":"rectangle","x":80,"y":50,"width":20,"height":20,"frameId":"other","strokeColor":"#2f9e44","backgroundColor":"#2f9e44","fillStyle":"solid","roughness":0,"updated":1},
            {"id":"outside","type":"rectangle","x":300,"y":300,"width":20,"height":20,"strokeColor":"#9c36b5","backgroundColor":"#9c36b5","fillStyle":"solid","roughness":0,"updated":1}
        ],
        "appState":{"frameRendering":{"enabled":false,"clip":false,"name":true,"outline":true}},
        "files":{}
    }"##)
    .expect("frame scene is admitted");
    let target = scene.frames().next().expect("target frame exists");
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .include_background(false)
        .build()
        .expect("frame request is valid");

    let outcome = Renderer::new()
        .render_frame(target, &request)
        .expect("target frame renders");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");

    assert!(svg.contains("viewBox=\"0 0 100 80\""));
    assert!(svg.contains("#e03131"));
    assert!(svg.contains("#1971c2"));
    assert!(!svg.contains("#2f9e44"));
    assert!(!svg.contains("#9c36b5"));
    assert!(!svg.contains("data-frame-name"));
    assert!(!svg.contains("data-frame-outline"));
}

#[test]
fn frame_export_keeps_overlapping_compatibility_placeholders() {
    let scene = SceneDocument::parse(
        br#"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"target","type":"frame","x":0,"y":0,"width":100,"height":80,"updated":1},
            {"id":"future","type":"future","x":10,"y":10,"width":80,"height":20,"updated":1},
            {"id":"outside","type":"future","x":200,"y":200,"width":30,"height":20,"updated":1}
        ],
        "appState":{},"files":{}
    }"#,
    )
    .expect("forward-compatible frame scene is admitted");
    let frame = scene.frames().next().expect("target frame exists");
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("frame request is valid");

    let outcome = Renderer::new()
        .render_frame(frame, &request)
        .expect("placeholder keeps the frame meaningful");
    let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert_eq!(svg.matches("<clipPath id=\"fallback-").count(), 1);
    assert_eq!(svg.matches("<text ").count(), 1);
    assert!(!outcome.report().is_empty());
    assert!(outcome
        .report()
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.json_pointer().starts_with("/elements/1")));
}

#[test]
fn frame_asset_census_excludes_unreferenced_supplements() {
    let scene = SceneDocument::parse(
        br#"{
            "type":"excalidraw","version":2,
            "elements":[{"id":"target","type":"frame","x":0,"y":0,"width":40,"height":30,"updated":1}],
            "appState":{},"files":{}
        }"#,
    )
    .expect("frame scene is admitted");
    let frame = scene.frames().next().expect("target frame exists");
    let assets = SupplementalAssets::builder()
        .asset("unreferenced", vec![0; 4])
        .build()
        .expect("supplement is valid");
    let limits = ResourceLimits::builder()
        .max_asset_bytes(3)
        .max_total_asset_bytes(3)
        .build()
        .expect("asset limits are valid");
    let request = FrameRenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .resource_limits(limits)
        .build()
        .expect("frame request is valid");

    Renderer::new()
        .render_frame(frame, &request)
        .expect("an unreferenced supplement is outside the target census");
}

#[test]
fn frame_requests_retain_only_controls_that_apply_to_frame_exports() {
    let metadata = MetadataOptions::new(true, true, true);
    let assets = SupplementalAssets::builder()
        .asset("image", vec![1, 2, 3])
        .build()
        .expect("supplemental assets are valid");
    let limits = ResourceLimits::builder()
        .max_output_bytes(1_024)
        .build()
        .expect("resource limits are valid");
    let request = FrameRenderRequest::builder(OutputFormat::Pdf)
        .include_background(false)
        .sizing(Sizing::MaxDimension(640.0))
        .color_mode(ColorMode::Dark)
        .metadata_options(metadata.clone())
        .supplemental_assets(assets)
        .resource_limits(limits.clone())
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .expect("frame request is valid");

    assert_eq!(request.format(), OutputFormat::Pdf);
    assert!(!request.include_background());
    assert_eq!(request.sizing(), Sizing::MaxDimension(640.0));
    assert_eq!(request.color_mode(), ColorMode::Dark);
    assert_eq!(request.metadata_options(), &metadata);
    assert_eq!(request.supplemental_assets().len(), 1);
    assert_eq!(request.resource_limits(), &limits);
    assert_eq!(
        request.pdf_options().map(PdfOptions::raster_fallback),
        Some(PdfRasterFallback::Forbid)
    );
}
