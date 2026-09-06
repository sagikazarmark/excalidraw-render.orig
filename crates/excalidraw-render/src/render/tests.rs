use super::*;
use crate::{
    font_metadata::FamilyMetrics, CompatibilityDiagnostic, CompatibilityRevision, DiagnosticCode,
    DiagnosticEffect, DiagnosticSeverity, Renderer, ResourceLimits,
};
use base64::Engine as _;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use std::fs;
use std::path::Path;

const EMPTY_SCENE: &[u8] =
    br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;

#[test]
fn focused_latin_svg_font_subset_beats_same_revision_complete_chunk_control() {
    let scene = SceneDocument::parse(
        br##"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","x":20,"y":30,"width":90,"height":72,"angle":0.125,"strokeColor":"#1e66f5","opacity":70,"fontSize":20,"fontFamily":5,"text":"One two three\nFour","textAlign":"center","verticalAlign":"middle","lineHeight":1.2,"frameId":null,"updated":1}],"appState":{},"files":{}}"##,
    )
    .expect("focused Latin scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let candidate = Renderer::new()
        .render(&scene, &request)
        .expect("focused Latin candidate renders");
    let control = render_with_svg_font_payload(
        &crate::catalog::REVISION_1,
        &scene,
        &request,
        svg_fonts::Payload::CompleteChunks,
    )
    .expect("focused Latin complete-chunk control renders");

    assert_eq!(candidate.report(), control.report());
    assert_focused_svg_font_savings(&candidate, &control);
}

#[test]
fn multi_chunk_svg_font_subsets_beat_same_revision_complete_chunk_control() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","width":160,"height":40,"fontSize":20,"fontFamily":5,"text":"A","originalText":"A\u0416","updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("multi-chunk scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let candidate = Renderer::new()
        .render(&scene, &request)
        .expect("multi-chunk candidate renders");
    let control = render_with_svg_font_payload(
        &crate::catalog::REVISION_1,
        &scene,
        &request,
        svg_fonts::Payload::CompleteChunks,
    )
    .expect("multi-chunk complete-chunk control renders");

    assert_eq!(candidate.report(), control.report());
    let candidate_svg = std::str::from_utf8(candidate.artifact().bytes()).expect("SVG is UTF-8");
    let control_svg = std::str::from_utf8(control.artifact().bytes()).expect("SVG is UTF-8");
    assert_eq!(candidate_svg.matches("font-family:'Excalifont'").count(), 2);
    assert_eq!(control_svg.matches("font-family:'Excalifont'").count(), 2);
    assert_focused_svg_font_savings(&candidate, &control);
}

#[test]
fn complete_cjk_svg_font_subsets_beat_same_revision_complete_chunk_control() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","width":60,"height":30,"fontSize":20,"fontFamily":5,"text":"\u6f22\u6f22A","updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("complete-CJK scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let candidate = Renderer::new()
        .render(&scene, &request)
        .expect("complete-CJK candidate renders");
    let control = render_with_svg_font_payload(
        &crate::catalog::REVISION_1,
        &scene,
        &request,
        svg_fonts::Payload::CompleteChunks,
    )
    .expect("complete-CJK complete-chunk control renders");

    assert_eq!(candidate.report(), control.report());
    for artifact in [&candidate, &control] {
        let svg = std::str::from_utf8(artifact.artifact().bytes()).expect("SVG is UTF-8");
        let xiaolai = svg
            .find("font-family:'Xiaolai'")
            .expect("Xiaolai chunk is selected");
        let excalifont = svg
            .find("font-family:'Excalifont'")
            .expect("Excalifont Latin chunk is selected");
        assert!(xiaolai < excalifont);
    }
    assert_focused_svg_font_savings(&candidate, &control);
}

#[test]
fn full_intersection_svg_font_subset_does_not_grow_against_complete_chunk_control() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","width":160,"height":40,"fontSize":20,"fontFamily":5,"text":"A","originalText":"A\u0100\u0416\u03b1\u02c7\u0462\u0300","updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("full-intersection scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");

    let candidate = Renderer::new()
        .render(&scene, &request)
        .expect("full-intersection candidate renders");
    let repeated = Renderer::new()
        .render(&scene, &request)
        .expect("full-intersection candidate renders repeatedly");
    let control = render_with_svg_font_payload(
        &crate::catalog::REVISION_1,
        &scene,
        &request,
        svg_fonts::Payload::CompleteChunks,
    )
    .expect("full-intersection complete-chunk control renders");

    assert_eq!(candidate.artifact().bytes(), repeated.artifact().bytes());
    assert_eq!(candidate.report(), control.report());
    assert_eq!(
        candidate.compatibility_revision(),
        CompatibilityRevision::REVISION_1
    );
    assert_eq!(
        control.compatibility_revision(),
        CompatibilityRevision::REVISION_1
    );
    let candidate_svg = std::str::from_utf8(candidate.artifact().bytes()).expect("SVG is UTF-8");
    let control_svg = std::str::from_utf8(control.artifact().bytes()).expect("SVG is UTF-8");
    assert_eq!(candidate_svg.matches("font-family:'Excalifont'").count(), 7);
    assert_eq!(control_svg.matches("font-family:'Excalifont'").count(), 7);
    assert!(candidate_svg.contains("svg-font-subset@sha256:"));
    assert!(!control_svg.contains("svg-font-subset@sha256:"));
    assert!(candidate.artifact().bytes().len() <= control.artifact().bytes().len());

    let mut expected_records = excalidraw_render_assets_base::revision_1()
        .assets
        .iter()
        .filter(|record| record.family == "Excalifont")
        .collect::<Vec<_>>();
    expected_records.sort_unstable_by_key(|record| {
        excalidraw_render_assets_base::font_face_order("Excalifont", record.upstream_path)
            .expect("Excalifont chunk has a pinned order")
    });
    let expected = expected_records
        .into_iter()
        .map(|record| {
            excalidraw_render_assets_base::load_asset(record.upstream_path)
                .expect("official Excalifont chunk is readable")
                .expect("official Excalifont chunk is present")
                .bytes()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let actual = embedded_test_woff2(control_svg);
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert!(
            actual == expected,
            "complete-chunk control payload {index} differs from its official source"
        );
    }
}

#[test]
fn font_subset_representative_direct_svg_corpus_does_not_grow_against_complete_chunk_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/qualification/svg-font-subset-size-corpus.json"
    )))
    .expect("representative SVG font subset size corpus is JSON");
    assert_eq!(
        corpus["schema"],
        "excalidraw-render/svg-font-subset-size-corpus/v1"
    );
    let cases = corpus["cases"]
        .as_array()
        .expect("representative size corpus cases are an array");
    assert!(!cases.is_empty());
    assert!(cases.iter().any(|case| case["layer"] == "representative"));
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    for case in cases {
        let id = case["id"].as_str().expect("size case has an ID");
        let layer = case["layer"]
            .as_str()
            .expect("size case has a conformance layer");
        assert!(matches!(layer, "focused" | "representative"));
        let path = case["scene"].as_str().expect("size case has a scene path");
        let scene_bytes = fs::read(root.join(path))
            .unwrap_or_else(|error| panic!("failed to read size case {id}: {error}"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&scene_bytes)),
            case["scene_sha256"]
                .as_str()
                .expect("size case has a scene digest"),
            "size case {id} scene digest",
        );
        let scene = SceneDocument::parse(&scene_bytes)
            .unwrap_or_else(|error| panic!("failed to parse size case {id}: {error}"));
        let candidate = Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("subset size case {id} failed: {error}"));
        let control = render_with_svg_font_payload(
            &crate::catalog::REVISION_1,
            &scene,
            &request,
            svg_fonts::Payload::CompleteChunks,
        )
        .unwrap_or_else(|error| panic!("complete-chunk size case {id} failed: {error}"));
        assert_eq!(candidate.report(), control.report(), "size case {id}");
        assert!(
            candidate.artifact().bytes().len() <= control.artifact().bytes().len(),
            "representative size case {id} grew: candidate={}, control={}",
            candidate.artifact().bytes().len(),
            control.artifact().bytes().len(),
        );
    }
}

fn assert_focused_svg_font_savings(candidate: &RenderOutcome, control: &RenderOutcome) {
    assert_eq!(
        candidate.compatibility_revision(),
        CompatibilityRevision::REVISION_1
    );
    assert_eq!(
        control.compatibility_revision(),
        CompatibilityRevision::REVISION_1
    );
    let candidate_len = candidate.artifact().bytes().len();
    let control_len = control.artifact().bytes().len();
    let savings = control_len.checked_sub(candidate_len).unwrap_or_else(|| {
        panic!("subset artifact grew: candidate={candidate_len} bytes, control={control_len} bytes")
    });
    assert!(
        savings > 4 * 1_024,
        "subset saved only {savings} bytes: candidate={candidate_len}, control={control_len}"
    );
    assert!(
        candidate_len * 5 < control_len * 4,
        "subset was not more than 20% smaller: candidate={candidate_len}, control={control_len}"
    );
}

fn embedded_test_woff2(svg: &str) -> Vec<Vec<u8>> {
    const PREFIX: &str = "data:font/woff2;base64,";
    let mut fonts = Vec::new();
    let mut remaining = svg;
    while let Some((_, tail)) = remaining.split_once(PREFIX) {
        let (encoded, rest) = tail.split_once(')').expect("font data URL terminates");
        fonts.push(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .expect("font data URL is valid base64"),
        );
        remaining = rest;
    }
    fonts
}

#[test]
fn focused_pdf_expectations_are_approval_bound() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace");
    let path = root.join(concat!(
        "tests/conformance/cases/focused/rectangle-outline/",
        "pdf-renderer-expectations.json"
    ));
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("PDF expectation manifest exists"))
            .expect("PDF expectation manifest is valid JSON");
    assert_eq!(manifest["case_id"], "focused.rectangle-outline.v1");
    assert_eq!(manifest["format"], "pdf");
    assert_eq!(manifest["compatibility_revision"], 1);
    assert_eq!(manifest["variant"], "default");
    assert_eq!(manifest["regions"], json!(["whole"]));
    assert_eq!(manifest["visual_class"], "geometry/image");
    assert_eq!(manifest["crops"], json!([]));
    assert_eq!(manifest["coverage"].as_array().map(Vec::len), Some(4));
    assert_eq!(
        manifest["case_manifest_approval_subject_sha256"],
        "sha256:bc870d20627148871b0b2c18dc9fe67476001dc9f47aa5f0b46742e5be837008"
    );

    let approval = manifest
        .as_object_mut()
        .expect("PDF expectation manifest is an object")
        .remove("approval")
        .expect("PDF expectation manifest is approved");
    assert_eq!(approval["approved_by"], "sagikazarmark");
    assert_eq!(approval["approved_at"], "2026-09-05");
    assert_eq!(
        approval["approval_url"],
        "https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5553306555"
    );
    let mut canonical = serde_json::to_vec(&manifest).expect("unsigned manifest serializes");
    canonical.push(b'\n');
    assert_eq!(
        approval["approval_subject_sha256"],
        format!(
            "sha256:{}",
            crate::resource_accounting::sha256_hex(&canonical)
        )
    );

    let files = manifest["files"]
        .as_array()
        .expect("PDF expectation files are an array");
    assert_eq!(files.len(), 13);
    for descriptor in files {
        let path = descriptor["path"]
            .as_str()
            .expect("PDF expectation path is a string");
        let bytes = fs::read(root.join(path)).expect("PDF expectation dependency exists");
        assert_eq!(
            descriptor["sha256"],
            crate::resource_accounting::sha256_hex(&bytes),
            "stale PDF expectation dependency {path}"
        );
    }

    let exception: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(concat!(
            "tests/conformance/cases/focused/rectangle-outline/",
            "pdf-visual-exception.json"
        )))
        .expect("PDF visual exception exists"),
    )
    .expect("PDF visual exception is valid JSON");
    let keys: Vec<_> = exception
        .as_object()
        .expect("PDF visual exception is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "approved_issue",
            "authority",
            "case_id",
            "format",
            "max_changed_pixels",
            "minimum_ssim_bits",
            "reason",
            "region",
            "schema",
            "variant",
            "viewer_profile",
            "visual_class",
        ]
    );
    assert_eq!(exception["case_id"], "focused.rectangle-outline.v1");
    assert_eq!(exception["variant"], "default");
    assert_eq!(exception["format"], "pdf");
    assert_eq!(exception["viewer_profile"], "canonical-pdf.pdfium.v1");
    assert_eq!(exception["region"], "whole");
    assert_eq!(exception["visual_class"], "geometry/image");
    assert_eq!(exception["max_changed_pixels"], 0);
    assert_eq!(exception["minimum_ssim_bits"], "3fefec69adc2369e");
}

#[test]
fn diagnostic_transaction_uses_fatal_precedence_and_never_appends_a_rejection() {
    let cases = [
        (1, 640, 8, ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED),
        (2, 640, 8, ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED),
        (2, 1_280, 8, ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED),
    ];

    for (max_diagnostics, max_working_bytes, max_total_work_units, expected_code) in cases {
        let limits = ResourceLimits::builder()
            .max_diagnostics(max_diagnostics)
            .max_working_bytes(max_working_bytes)
            .max_element_work_units(1)
            .max_asset_work_units(1)
            .max_total_work_units(max_total_work_units)
            .build()
            .expect("diagnostic precedence limits are valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("diagnostic request is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");
        let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
        let mut session = RenderSession::new(
            profile,
            crate::catalog::REVISION_1.scene_surface(),
            &scene,
            &request,
        );
        let diagnostic = test_diagnostic();

        session
            .retain_diagnostic(
                &request,
                ResourcePhase::JsonTraversalAndNormalization,
                "test_diagnostic",
                diagnostic.clone(),
            )
            .expect("the first diagnostic fits at equality");
        let error = session
            .retain_diagnostic(
                &request,
                ResourcePhase::JsonTraversalAndNormalization,
                "test_diagnostic",
                diagnostic.clone(),
            )
            .expect_err("the second diagnostic crosses an aggregate limit");

        assert_eq!(error.code(), expected_code);
        let mut retained = diagnostic;
        retained.order.owner_index = scene.source_owner_order("elements", 0);
        retained.order.retention_ordinal = 0;
        assert_eq!(error.report().diagnostics(), &[retained]);
    }
}

#[test]
fn later_output_exhaustion_preserves_the_accumulated_report() {
    let limits = ResourceLimits::builder()
        .max_output_bytes(1)
        .build()
        .expect("output limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let mut session = RenderSession::new(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );
    let diagnostic = test_diagnostic();
    session
        .retain_diagnostic(
            &request,
            ResourcePhase::ProjectionAndFallback,
            "test_diagnostic",
            diagnostic.clone(),
        )
        .expect("diagnostic is retained");

    let error = session
        .render(&scene, &request, None, svg_fonts::Payload::Subset)
        .expect_err("output exhaustion is fatal");

    assert_eq!(error.code(), ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED);
    let mut retained = diagnostic;
    retained.order.owner_index = scene.source_owner_order("elements", 0);
    retained.order.retention_ordinal = 0;
    assert_eq!(error.report().diagnostics(), &[retained]);
}

#[test]
fn diagnostic_trace_uses_the_callers_canonical_phase() {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let mut session = RenderSession::new_with_trace(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );

    session
        .retain_diagnostic(
            &request,
            ResourcePhase::JsonTraversalAndNormalization,
            "normalization_diagnostic",
            test_diagnostic(),
        )
        .expect("diagnostic is retained");

    let trace: serde_json::Value =
        serde_json::from_slice(&session.meter.trace_bytes()).expect("trace is valid JSON");
    assert_eq!(
        trace["entries"][0]["phase"],
        "json_traversal_and_normalization"
    );
    assert_eq!(trace["entries"][0]["event"], "normalization_diagnostic");
}

#[test]
fn local_recovery_trace_commits_only_the_aggregate_observation() {
    let source = br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"text","type":"text","x":0,"y":0,"width":100,"height":30,"strokeColor":"#000000","opacity":100,"fontSize":20,"fontFamily":5,"text":"ab","textAlign":"left","verticalAlign":"top","lineHeight":1.2,"updated":1}],
        "appState":{},"files":{}
    }"##;
    let scene = SceneDocument::parse(source).expect("text vector is valid");
    let limits = ResourceLimits::builder()
        .max_text_scalars_per_element(1)
        .build()
        .expect("local text limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (result, trace) = render_with_trace(profile, &scene, &request);
    result.expect("local text exhaustion is recoverable");
    let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
    let rejection = trace["entries"]
        .as_array()
        .expect("trace entries are an array")
        .iter()
        .find(|entry| entry["decision"] == "rejected_local")
        .expect("local rejection is represented in the trace");

    assert_eq!(rejection["event"], "text_scalar_and_run_preflight");
    assert_eq!(rejection["deltas"]["text_scalars"], 2);
    assert_eq!(rejection["deltas"]["working_bytes"], 0);
    assert_eq!(rejection["deltas"]["total_work_units"], 0);
    assert_eq!(rejection["after"]["text_scalars"], 2);
}

#[test]
fn canonical_resource_trace_is_uncharged_stable_and_semantic() {
    let expected_digests = [
        (
            OutputFormat::Svg,
            "e314b25a1c82ee51f290fa67e00d53143ca41d8478d57824b494d9306832753a",
        ),
        (
            OutputFormat::Png,
            "597955fc8a6149633ed5f9fbf475f1ac19b3abe05421c0d1cf4dcea99dced322",
        ),
        (
            OutputFormat::Pdf,
            "9f2c32bbb479856363721973d7e60b0cfac1688952cdb2eebdf46f245ee31089",
        ),
    ];

    for (format, expected_digest) in expected_digests {
        let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");
        let ordinary = Renderer::new()
            .render(&scene, &request)
            .expect("ordinary render succeeds");

        let (first_result, first_trace) = render_with_trace(profile, &scene, &request);
        let (second_result, second_trace) = render_with_trace(profile, &scene, &request);
        let first = first_result.expect("traced render succeeds");
        let second = second_result.expect("repeated traced render succeeds");

        assert_eq!(first_trace, second_trace);
        assert_eq!(first.artifact().bytes(), ordinary.artifact().bytes());
        assert_eq!(second.artifact().bytes(), ordinary.artifact().bytes());
        assert_eq!(first.report(), ordinary.report());
        assert_eq!(second.report(), ordinary.report());

        let trace_text = std::str::from_utf8(&first_trace).expect("trace is UTF-8 JSON");
        for forbidden in ["allocator", "backend", "cache", "thread", "timing"] {
            assert!(
                !trace_text.contains(forbidden),
                "trace exposes forbidden implementation fact {forbidden}"
            );
        }
        let trace: serde_json::Value =
            serde_json::from_slice(&first_trace).expect("trace schema is valid JSON");
        assert_eq!(trace["schema_version"], 1);
        assert_eq!(trace["header"]["compatibility_revision"], 1);
        assert_eq!(trace["header"]["format"], format.as_str());
        assert_eq!(trace["header"]["limits"]["max_scene_bytes"], 33_554_432);
        assert_eq!(
            trace["header"]["input_digests"]["supplemental_assets"],
            json!([])
        );

        let entries = trace["entries"]
            .as_array()
            .expect("trace entries are an array");
        assert!(entries.iter().all(|entry| {
            matches!(
                entry["decision"].as_str(),
                Some(
                    "committed"
                        | "rejected_local"
                        | "rejected_fatal"
                        | "shared_intrinsic"
                        | "released"
                )
            )
        }));
        assert_eq!(entries[0]["event"], "request_session");
        assert_eq!(entries[1]["event"], "scene_source_octets");
        assert_eq!(entries[2]["event"], "json_node");
        assert_eq!(entries[2]["json_pointer"], "");

        let digest = crate::resource_accounting::sha256_hex(&first_trace);
        assert_eq!(
            digest, expected_digest,
            "canonical {format:?} trace changed"
        );
    }
}

#[test]
fn direct_svg_font_subset_trace_follows_semantic_selection_and_derivation_order() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","width":80,"height":30,"fontSize":20,"fontFamily":5,"text":"A","originalText":"A\u0416","updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("font subset trace scene is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("SVG request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (result, trace) = render_with_trace(profile, &scene, &request);
    result.expect("font subset render succeeds");
    let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
    let entries = trace["entries"].as_array().expect("entries are an array");
    let events = entries
        .iter()
        .filter_map(|entry| entry["event"].as_str())
        .collect::<Vec<_>>();
    let family = events
        .iter()
        .position(|event| *event == "svg_font_family_scalar_set")
        .expect("family scalar set is traced");
    let first_considered = events
        .iter()
        .position(|event| *event == "svg_font_chunk_considered")
        .expect("candidate chunks are traced");
    let first_selected = events
        .iter()
        .position(|event| *event == "svg_font_chunk_selected")
        .expect("selected chunks are traced");
    let envelope = events
        .iter()
        .position(|event| *event == "svg_font_dependency_envelope")
        .expect("the dependency envelope is traced");
    let decode = events
        .iter()
        .position(|event| *event == "svg_font_woff2_decode")
        .expect("decode is traced");
    let css = events
        .iter()
        .rposition(|event| *event == "svg_font_css_data_url")
        .expect("CSS data URL is traced");
    assert!(family < first_considered);
    assert!(first_considered < first_selected);
    assert!(first_selected < envelope);
    assert!(envelope < decode);
    assert!(decode < css);
    assert!(!events.contains(&"svg_font_subset_workspace_preflight"));
    assert_eq!(
        events
            .iter()
            .zip(entries)
            .filter(|(event, entry)| {
                **event == "svg_font_chunk_considered" && entry["decision"] == "committed"
            })
            .count(),
        7
    );
    assert_eq!(
        entries
            .iter()
            .filter(|entry| {
                entry["event"] == "svg_font_chunk_selected" && entry["decision"] == "committed"
            })
            .count(),
        2
    );

    let envelope_entry = &entries[envelope];
    assert!(envelope_entry["deltas"]["working_bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert!(envelope_entry["deltas"]["total_work_units"]
        .as_u64()
        .is_some_and(|work| work > 0));
    let crossing = envelope_entry["attempted"]["working_bytes"]
        .as_u64()
        .expect("the attempted envelope has working bytes")
        - 1;
    let limited_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(
            ResourceLimits::builder()
                .max_working_bytes(crossing)
                .build()
                .expect("the envelope crossing limit is valid"),
        )
        .build()
        .expect("the envelope crossing request is valid");
    let (limited_result, limited_trace) = render_with_trace(profile, &scene, &limited_request);
    assert_eq!(
        limited_result
            .expect_err("the envelope crosses before derivation")
            .code(),
        ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED
    );
    let limited_trace: serde_json::Value =
        serde_json::from_slice(&limited_trace).expect("the limited trace is valid JSON");
    let limited_entries = limited_trace["entries"]
        .as_array()
        .expect("limited entries are an array");
    let rejected_envelope = limited_entries
        .iter()
        .position(|entry| {
            entry["event"] == "svg_font_dependency_envelope"
                && entry["decision"] == "rejected_fatal"
        })
        .expect("the dependency envelope is rejected");
    assert!(!limited_entries
        .iter()
        .any(|entry| entry["event"] == "svg_font_woff2_decode"));
    assert!(limited_entries[rejected_envelope + 1..]
        .iter()
        .all(|entry| entry["decision"] == "released"));

    let output_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(
            ResourceLimits::builder()
                .max_output_bytes(1)
                .build()
                .expect("the output crossing limit is valid"),
        )
        .build()
        .expect("the output crossing request is valid");
    let (output_result, output_trace) = render_with_trace(profile, &scene, &output_request);
    assert_eq!(
        output_result
            .expect_err("output rejection occurs after derivation")
            .code(),
        ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED
    );
    let output_trace: serde_json::Value =
        serde_json::from_slice(&output_trace).expect("the output trace is valid JSON");
    let output_entries = output_trace["entries"]
        .as_array()
        .expect("output entries are an array");
    let workspace_release = output_entries
        .iter()
        .position(|entry| {
            entry["event"] == "svg_serializer_workspace" && entry["decision"] == "released"
        })
        .expect("serializer workspace is released after failure");
    let envelope_release = output_entries
        .iter()
        .position(|entry| {
            entry["event"] == "svg_font_dependency_envelope" && entry["decision"] == "released"
        })
        .expect("dependency envelope is released after failure");
    assert!(workspace_release < envelope_release);
}

#[test]
fn cjk_subset_trigger_matches_script_and_explicit_symbol_classes() {
    assert!(is_cjk(0x4e2d));
    assert!(is_cjk(0xd7b0));
    assert!(is_cjk(u32::from('￥')));
    assert!(!is_cjk(0x2ff0));
    assert!(!is_cjk(u32::from('A')));
}

#[test]
fn png_frame_clip_masks_are_charged_once_per_frame_and_owned_by_no_child() {
    // Two frames with two children each, plus one unframed rectangle. With
    // names and outlines off the canvas is 250x80; the frame clips cover
    // 100x60 and 80x40 device pixels.
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"left","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
            {"id":"right","type":"frame","x":150,"y":0,"width":80,"height":40,"updated":1},
            {"id":"l1","type":"rectangle","x":10,"y":10,"width":20,"height":20,"frameId":"left","roughness":0,"updated":1},
            {"id":"r1","type":"rectangle","x":160,"y":10,"width":20,"height":20,"frameId":"right","roughness":0,"updated":1},
            {"id":"l2","type":"rectangle","x":40,"y":10,"width":20,"height":20,"frameId":"left","roughness":0,"updated":1},
            {"id":"r2","type":"rectangle","x":190,"y":10,"width":20,"height":20,"frameId":"right","roughness":0,"updated":1},
            {"id":"free","type":"rectangle","x":110,"y":45,"width":20,"height":10,"roughness":0,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("two-frame scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Png)
        .frame_options(crate::FrameOptions::new(true, true, false, false))
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (result, trace) = render_with_trace(profile, &scene, &request);
    result.expect("two-frame PNG renders");
    let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
    let entries = trace["entries"].as_array().expect("entries are an array");

    let mask_samples: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|entry| entry["event"] == "frame_clip_mask_samples")
        .collect();
    let expected = [
        ("left", "/elements/0", 100 * 60),
        ("right", "/elements/1", 80 * 40),
    ];
    assert_eq!(mask_samples.len(), expected.len());
    for (entry, (frame_id, pointer, pixels)) in mask_samples.iter().zip(expected) {
        assert_eq!(entry["element_id"], frame_id);
        assert_eq!(entry["json_pointer"], pointer);
        assert_eq!(entry["local_owner"], serde_json::Value::Null);
        assert_eq!(entry["deltas"]["rasterized_pixels"], pixels);
        assert_eq!(entry["deltas"]["total_work_units"], 8 * pixels);
        assert_eq!(entry["decision"], "committed");
    }

    let mask_surfaces: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|entry| {
            entry["event"] == "frame_clip_mask_surface" && entry["record_action"] == "acquire"
        })
        .collect();
    assert_eq!(mask_surfaces.len(), expected.len());
    for (entry, (_, _, pixels)) in mask_surfaces.iter().zip(expected) {
        assert_eq!(entry["record_kind"], "raster_mask_surface");
        assert_eq!(entry["deltas"]["working_bytes"], 256 + pixels);
    }

    let last_mask = entries
        .iter()
        .rposition(|entry| entry["event"] == "frame_clip_mask_samples")
        .expect("frame masks are traced");
    let first_projection = entries
        .iter()
        .position(|entry| entry["event"] == "vector_coverage_samples")
        .expect("element projection is traced");
    assert!(last_mask < first_projection);
    assert!(
        !entries
            .iter()
            .any(|entry| entry["event"] == "paint_mask_samples"),
        "plain framed children need no mask of their own"
    );
}

#[test]
fn frame_name_is_laid_out_as_the_reference_text_element() {
    let font = text::FallbackFont::load()
        .ok()
        .expect("the bundled Liberation Sans loads");
    let baseline = FamilyMetrics::for_source_family(2)
        .expect("Helvetica has metadata metrics")
        .first_baseline(14.0, 17.5);
    assert!((baseline - 12.530_273_437_5).abs() < 1e-12);

    // A name narrower than its frame keeps its measured width and starts at
    // the frame's x.
    let name = font
        .resolve_frame_name("Clip", 580.0)
        .ok()
        .expect("the name shapes");
    assert_eq!(name.label.text, "Clip");
    assert_eq!(name.label.display_text, "Clip");
    assert!(name.width > 20.0 && name.width < 30.0, "{}", name.width);
    assert_eq!(name.label.glyphs.len(), 4);
    assert!(name.label.glyphs[0].x.abs() < 1e-12, "left aligned");
    for glyph in &name.label.glyphs {
        assert!((glyph.y - baseline).abs() < 1e-12, "baseline {}", glyph.y);
    }
    assert!((name.label.baseline - baseline).abs() < 1e-12);

    // Wider than the frame: the longest prefix that fits with `...` appended,
    // the text element as wide as the frame.
    let full = font
        .resolve_frame_name("A rather long frame name", 1_000.0)
        .ok()
        .expect("the full name shapes");
    let narrow = font
        .resolve_frame_name("A rather long frame name", full.width / 2.0)
        .ok()
        .expect("the truncated name shapes");
    assert_eq!(narrow.width, full.width / 2.0);
    let kept = narrow
        .label
        .text
        .strip_suffix("...")
        .expect("ASCII dots, as truncateText appends");
    assert!("A rather long frame name".starts_with(kept) && !kept.is_empty());
    let fits = |text: &str| {
        font.resolve_frame_name(text, f64::INFINITY)
            .ok()
            .expect("candidate shapes")
            .width
    };
    assert!(fits(&narrow.label.text) <= narrow.width, "the cut fits");
    let one_more = format!(
        "{}...",
        "A rather long frame name"
            .chars()
            .take(kept.chars().count() + 1)
            .collect::<String>()
    );
    assert!(
        fits(&one_more) > narrow.width,
        "and is the longest that does"
    );

    // Too narrow for even one character with the dots: the whole name stays,
    // as truncateText leaves it, at the frame's width.
    let tiny = font
        .resolve_frame_name("Clip", 1.0)
        .ok()
        .expect("the tiny frame's name shapes");
    assert_eq!(tiny.label.text, "Clip");
    assert_eq!(tiny.width, 1.0);

    // A blank name measures as one space, as measureText does.
    let blank = font
        .resolve_frame_name("", 100.0)
        .ok()
        .expect("the blank name shapes");
    assert!(blank.label.glyphs.is_empty());
    assert!(blank.width > 0.0);
}

/// The Excalifont layout the line-ending tests share: 20 px at line height
/// 1.25, so lines sit 25 px apart.
fn excalifont_layout() -> text::TextLayout<'static> {
    text::TextLayout {
        width: 100.0,
        font_size: 20.0,
        line_height: 1.25,
        text_align: "left",
        font_family: 5,
    }
}

/// `(source, display)` per laid-out line: what `data-source-text` and
/// `ActualText` carry beside what the glyphs show.
fn line_texts(laid_out: &text::ResolvedText) -> Vec<(&str, &str)> {
    laid_out
        .lines
        .iter()
        .map(|line| (line.text.as_str(), line.display_text.as_str()))
        .collect()
}

#[test]
fn a_carriage_return_ends_a_line_like_the_reference_and_stays_in_the_source_text() {
    // The reference renders `element.text.replace(/\r\n?/g, "\n").split("\n")`
    // and leaves `text` alone, so `\r\n` and `\r` each end a line while the
    // source bytes round-trip through the per-line source text (#110).
    let font = text::OfficialFont::load(5, "a\r\nb")
        .ok()
        .expect("Excalifont loads");
    let laid_out = font
        .layout("a\r\nb", excalifont_layout())
        .ok()
        .expect("CRLF text lays out");
    assert_eq!(line_texts(&laid_out), [("a\r\n", "a"), ("b", "b")]);
    assert!((laid_out.lines[1].baseline - laid_out.lines[0].baseline - 25.0).abs() < 1e-9);
    assert!(laid_out.missing_codepoints.is_empty(), "\\r is not a glyph");
    assert_eq!(laid_out.line_break_steps, 2);
    assert_eq!(laid_out.prepaid_line_break_steps, 2);
    assert_eq!(laid_out.prepaid_glyphs, 2);
    assert_eq!(laid_out.glyph_count(), 2);
}

#[test]
fn every_line_ending_shape_splits_as_replace_then_split_does() {
    // Worked through `text.replace(/\r\n?/g, "\n").split("\n")`: `\r\n` is one
    // break, a bare `\r` is one, `\n\r` and `\r\r` are two each, a trailing
    // terminator leaves an empty last line, and empty text is one empty line.
    let font = text::OfficialFont::load(5, "ab")
        .ok()
        .expect("Excalifont loads");
    let cases: [(&str, &[(&str, &str)]); 7] = [
        ("a\rb", &[("a\r", "a"), ("b", "b")]),
        ("a\n\rb", &[("a\n", "a"), ("\r", ""), ("b", "b")]),
        ("a\r\r\nb", &[("a\r", "a"), ("\r\n", ""), ("b", "b")]),
        ("a\r", &[("a\r", "a"), ("", "")]),
        ("a\r\n", &[("a\r\n", "a"), ("", "")]),
        ("\r", &[("\r", ""), ("", "")]),
        ("", &[("", "")]),
    ];
    for (source, expected) in cases {
        let laid_out = font
            .layout(source, excalifont_layout())
            .ok()
            .unwrap_or_else(|| panic!("{source:?} lays out"));
        assert_eq!(line_texts(&laid_out), expected, "{source:?}");
        assert!(laid_out.missing_codepoints.is_empty(), "{source:?}");
        let lines = u64::try_from(expected.len()).expect("small");
        assert_eq!(laid_out.line_break_steps, lines, "{source:?}");
        assert_eq!(laid_out.prepaid_line_break_steps, lines, "{source:?}");
        assert_eq!(
            laid_out.prepaid_glyphs,
            laid_out.glyph_count(),
            "{source:?} prepays exactly its shaped glyphs"
        );
        // The per-line source text reassembles the stored text byte for byte.
        let reassembled = laid_out
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<String>();
        assert_eq!(reassembled, source);
    }
}

#[test]
fn the_fallback_layout_splits_lines_by_the_same_rule() {
    // Missing glyphs and bidirectional text take `layout_with_fallbacks`; a
    // `\r` there must end the line too rather than reach the fallback probes.
    let mut fonts = vec![text::OfficialFont::load(5, "ab")
        .ok()
        .expect("Excalifont loads")];
    for (source, expected) in [
        ("a\r\nb", vec![("a\r\n", "a"), ("b", "b")]),
        ("a\rb\r", vec![("a\r", "a"), ("b\r", "b"), ("", "")]),
        // CJK is not in Excalifont, so each line here is a Xiaolai fallback run.
        ("中\r\n文", vec![("中\r\n", "中"), ("文", "文")]),
    ] {
        let (laid_out, _) = text::layout_with_fallbacks(0, &mut fonts, source, excalifont_layout())
            .ok()
            .unwrap_or_else(|| panic!("{source:?} lays out with fallbacks"));
        assert_eq!(line_texts(&laid_out), expected, "{source:?}");
        assert!(laid_out.missing_codepoints.is_empty(), "{source:?}");
        assert!(!laid_out.noto_emoji_substituted, "{source:?}");
        let lines = u64::try_from(expected.len()).expect("small");
        assert_eq!(laid_out.line_break_steps, lines, "{source:?}");
        assert_eq!(laid_out.prepaid_line_break_steps, lines, "{source:?}");
        assert_eq!(
            laid_out.prepaid_glyphs,
            laid_out.glyph_count(),
            "{source:?}"
        );
        for (index, line) in laid_out.lines.iter().enumerate() {
            let expected_baseline = laid_out.lines[0].baseline + index as f64 * 25.0;
            assert!(
                (line.baseline - expected_baseline).abs() < 1e-9,
                "{source:?} line {index} baseline {}",
                line.baseline
            );
        }
    }
}

#[test]
fn line_terminators_do_not_steer_face_selection() {
    // Face selection asks whether every shaped scalar has a glyph; a `\r` is
    // never shaped, so it picks the face `\n` text would.
    for family in [1, 2, 3, 5, 6, 7, 8, 9] {
        let identity = |text: &str| {
            let (_, _, identity, _, _, _) = text::OfficialFont::load(family, text)
                .ok()
                .unwrap_or_else(|| panic!("family {family} loads for {text:?}"))
                .into_parts();
            identity
        };
        assert_eq!(identity("a\rb"), identity("a\nb"), "family {family} \\r");
        assert_eq!(
            identity("a\r\nb"),
            identity("a\nb"),
            "family {family} \\r\\n"
        );
    }
}

#[test]
fn required_raster_trace_precedes_projection_work() {
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (result, trace) = render_with_trace(profile, &scene, &request);
    result.expect("empty PNG renders");
    let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
    let entries = trace["entries"].as_array().expect("entries are an array");
    let main = entries
        .iter()
        .position(|entry| entry["event"] == "main_raster_surface")
        .expect("required main raster is traced");
    let background = entries
        .iter()
        .position(|entry| entry["event"] == "solid_background_samples")
        .expect("background projection is traced");

    assert!(main < background);
    assert_eq!(entries[main]["deltas"]["rasterized_pixels"], 400);
    assert_eq!(entries[main]["decision"], "committed");
}

#[test]
fn normalization_resource_events_repeat_with_generated_identity_and_source_order() {
    let source = br#"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"#;
    let scene = SceneDocument::parse(source).expect("normalization vector is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (first_result, first_trace) = render_with_trace(profile, &scene, &request);
    let (second_result, second_trace) = render_with_trace(profile, &scene, &request);
    let first = first_result.expect("normalization vector renders");
    let second = second_result.expect("repeated normalization vector renders");

    assert_eq!(first.artifact().bytes(), second.artifact().bytes());
    assert_eq!(first.report(), second.report());
    assert_eq!(first_trace, second_trace);

    let trace: serde_json::Value =
        serde_json::from_slice(&first_trace).expect("trace is valid JSON");
    let normalization_entries = trace["entries"]
        .as_array()
        .expect("trace entries are an array")
        .iter()
        .filter(|entry| {
            matches!(
                entry["event"].as_str(),
                Some("normalization_repair" | "normalized_rectangle")
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(normalization_entries.len(), 4);
    assert_eq!(
        normalization_entries
            .iter()
            .map(|entry| entry["json_pointer"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec![
            "/elements/0",
            "/elements/0/id",
            "/elements/0/seed",
            "/elements/0/updated",
        ]
    );
    assert!(normalization_entries.iter().all(|entry| {
        entry["element_id"]
            == "generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792"
    }));
    assert!(normalization_entries[1..].iter().all(|entry| {
        entry["deltas"]["diagnostics"] == 1 && entry["deltas"]["total_work_units"] == 8
    }));
    assert_eq!(
        normalization_entries[0]["deltas"]["total_work_units"],
        crate::normalization::NORMALIZATION_WORK_UNITS + 8
    );
}

#[test]
fn format_plan_and_semantic_records_keep_each_rectangle_owner() {
    let source = br#"{"type":"excalidraw","version":2,"elements":[{"id":"first","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"id":"second","type":"rectangle","x":50,"width":40,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"#;
    let scene = SceneDocument::parse(source).expect("multi-rectangle vector is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (result, trace) = render_with_trace(profile, &scene, &request);
    result.expect("multi-rectangle vector renders");
    let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
    let operations = trace["entries"]
        .as_array()
        .expect("trace entries are an array")
        .iter()
        .filter(|entry| entry["event"] == "element_operation" && entry["decision"] == "committed")
        .map(|entry| {
            (
                entry["element_id"].as_str().unwrap_or_default(),
                entry["json_pointer"].as_str().unwrap_or_default(),
                entry["deltas"]["total_work_units"]
                    .as_u64()
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        vec![("first", "/elements/0", 8), ("second", "/elements/1", 8),]
    );
}

#[test]
fn rejected_trace_entry_is_exact_and_does_not_commit_partial_output() {
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let limits = ResourceLimits::builder()
        .max_output_bytes(1)
        .build()
        .expect("output limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (result, trace_bytes) = render_with_trace(profile, &scene, &request);
    let error = result.expect_err("output exhaustion returns no artifact");
    assert_eq!(error.code(), ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED);
    assert!(error.report().is_empty());

    let trace: serde_json::Value =
        serde_json::from_slice(&trace_bytes).expect("trace is valid JSON");
    let entries = trace["entries"].as_array().expect("entries are an array");
    let rejection = entries
        .iter()
        .position(|entry| entry["decision"] == "rejected_fatal")
        .expect("trace has a rejected transaction");
    let rejected = &entries[rejection];
    assert_eq!(rejected["event"], "artifact_retention");
    assert_eq!(rejected["decision"], "rejected_fatal");
    assert_eq!(rejected["code"], "resource.output-bytes-exceeded");
    assert_eq!(rejected["measurement"], "exact");
    assert_eq!(rejected["before"], rejected["after"]);
    assert_eq!(rejected["after"]["output_bytes"], 0);
    assert!(rejected["attempted"]["output_bytes"]
        .as_u64()
        .is_some_and(|attempted| attempted > 1));
    assert!(entries[rejection + 1..]
        .iter()
        .all(|entry| entry["decision"] == "released"));
}

#[test]
fn rejected_output_measurement_never_runs_the_emission_pass() {
    let limits = ResourceLimits::builder()
        .max_output_bytes(1)
        .build()
        .expect("output limit is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(limits)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let mut session = RenderSession::new(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );
    let mut passes = 0;

    let error = session
        .measure_and_emit_output(&request, ErrorCode::ENCODE_SVG_FAILED, 0, || {
            passes += 1;
            Ok(b"measured output".to_vec())
        })
        .expect_err("measured output crosses the configured ceiling");

    assert_eq!(error.code(), ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED);
    assert_eq!(passes, 1, "rejected output is measured but never emitted");
    assert!(session.staged_output.is_none());
}

#[test]
fn emission_must_match_the_measured_artifact_bytes() {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let mut session = RenderSession::new(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );

    let error = session
        .emit_premeasured_output(
            &request,
            ErrorCode::ENCODE_SVG_FAILED,
            0,
            b"measured".to_vec(),
            || Ok(b"tampered".to_vec()),
        )
        .expect_err("same-length output from a divergent pass is not publishable");

    assert_eq!(error.code(), ErrorCode::RENDER_INTERNAL_FAILURE);
    assert!(session.staged_output.is_none());
}

#[test]
fn allocation_refusal_preserves_the_committed_trace_and_report_prefix() {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let mut successful = RenderSession::new_with_trace(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );
    let mut refused = RenderSession::new_with_trace(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );
    let diagnostic = test_diagnostic();
    refused
        .retain_diagnostic(
            &request,
            ResourcePhase::ProjectionAndFallback,
            "test_diagnostic",
            diagnostic.clone(),
        )
        .expect("report prefix is retained");
    successful
        .retain_diagnostic(
            &request,
            ResourcePhase::ProjectionAndFallback,
            "test_diagnostic",
            diagnostic.clone(),
        )
        .expect("control report prefix is retained");
    successful
        .emit_premeasured_output(
            &request,
            ErrorCode::ENCODE_SVG_FAILED,
            0,
            b"artifact".to_vec(),
            || Ok(b"artifact".to_vec()),
        )
        .expect("control emission succeeds");
    let expected_trace: serde_json::Value =
        serde_json::from_slice(&successful.meter.trace_bytes()).expect("control trace is JSON");

    let error = refused
        .emit_premeasured_output(
            &request,
            ErrorCode::ENCODE_SVG_FAILED,
            0,
            b"artifact".to_vec(),
            || {
                Err(FormatEncodeError::MemoryUnavailable {
                    subject_kind: "artifact",
                    subject: "svg_output",
                })
            },
        )
        .expect_err("injected physical allocation refusal is fatal");

    assert_eq!(error.code(), ErrorCode::RESOURCE_MEMORY_UNAVAILABLE);
    assert_eq!(error.report().diagnostics().len(), 1);
    assert!(refused.staged_output.is_none());
    let refused_trace: serde_json::Value =
        serde_json::from_slice(&refused.meter.trace_bytes()).expect("refusal trace is JSON");
    assert_eq!(refused_trace["header"], expected_trace["header"]);
    assert_eq!(refused_trace["entries"], expected_trace["entries"]);
}

#[test]
fn scene_carrier_failure_omits_only_the_scene_component() {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .metadata_options(crate::MetadataOptions::new(true, true, true))
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let mut session = RenderSession::new(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );

    let (metadata, _) = session
        .select_artifact_metadata(
            &scene,
            &request,
            None,
            ErrorCode::ENCODE_SVG_FAILED,
            0,
            |metadata| {
                if metadata.canonical_scene().is_some() {
                    Err(FormatEncodeError::Metadata(MetadataFailure {
                        code: DiagnosticCode::METADATA_SCENE_EMBEDDING_FAILED,
                        component: MetadataComponent::EmbeddedScene,
                        operation: MetadataOperation::Embedding,
                    }))
                } else {
                    Ok(metadata.manifest().unwrap_or_default().as_bytes().to_vec())
                }
            },
        )
        .expect("scene carrier failure is recoverable");

    assert!(metadata.canonical_scene().is_none());
    let manifest = metadata.manifest().expect("earlier metadata remains");
    assert!(manifest.contains("\"provenance\":{"));
    assert!(manifest.contains("\"diagnostic_summary\":{"));
    assert!(manifest.ends_with("\"embedded_scene\":null}"));
    assert_eq!(session.report.diagnostics.len(), 1);
    assert_eq!(
        session.report.diagnostics[0].code(),
        DiagnosticCode::METADATA_SCENE_EMBEDDING_FAILED
    );
}

#[test]
fn manifest_carrier_failure_without_notices_drops_all_optional_metadata() {
    let request = RenderRequest::builder(OutputFormat::Svg)
        .metadata_options(crate::MetadataOptions::new(false, true, false))
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let mut session = RenderSession::new(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );

    let (metadata, artifact) = session
        .select_artifact_metadata(
            &scene,
            &request,
            None,
            ErrorCode::ENCODE_SVG_FAILED,
            0,
            |metadata| {
                if metadata.manifest().is_some() {
                    Err(FormatEncodeError::Metadata(MetadataFailure {
                        code: DiagnosticCode::METADATA_MANIFEST_EMBEDDING_FAILED,
                        component: MetadataComponent::Manifest,
                        operation: MetadataOperation::Embedding,
                    }))
                } else {
                    Ok(b"required artifact".to_vec())
                }
            },
        )
        .expect("optional manifest carrier failure is recoverable");

    assert!(metadata.manifest().is_none());
    assert!(metadata.canonical_scene().is_none());
    assert_eq!(artifact, b"required artifact");
    assert_eq!(session.report.diagnostics.len(), 1);
    assert_eq!(
        session.report.diagnostics[0].code(),
        DiagnosticCode::METADATA_MANIFEST_EMBEDDING_FAILED
    );
}

#[test]
fn metadata_retention_rejection_preserves_measurement_work_in_the_trace() {
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");
    let metadata = crate::MetadataOptions::new(true, true, true);
    let unlimited_request = RenderRequest::builder(OutputFormat::Svg)
        .metadata_options(metadata.clone())
        .build()
        .expect("request is valid");
    let unlimited = Renderer::new()
        .render(&scene, &unlimited_request)
        .expect("control render succeeds");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .metadata_options(metadata)
        .resource_limits(
            ResourceLimits::builder()
                .max_output_bytes(u64::try_from(unlimited.artifact().bytes().len()).unwrap() - 1)
                .build()
                .expect("limits are valid"),
        )
        .build()
        .expect("request is valid");

    let (result, trace) = render_with_trace(profile, &scene, &request);
    result.expect("embedded scene retention exhaustion is recoverable");
    let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is JSON");
    let entries = trace["entries"].as_array().expect("entries are an array");
    let measurement = entries
        .iter()
        .position(|entry| entry["event"] == "embedded_scene_metadata_measurement")
        .expect("measurement work is committed");
    let rejection = entries
        .iter()
        .position(|entry| entry["event"] == "embedded_scene_metadata_retention")
        .expect("retention rejection is traced");

    assert!(measurement < rejection);
    assert_eq!(entries[measurement]["decision"], "committed");
    assert_eq!(entries[rejection]["decision"], "rejected_local");
    assert_eq!(entries[rejection]["before"], entries[rejection]["after"]);
}

#[test]
fn diagnosed_unbounded_omission_has_its_own_semantic_transaction() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[null],"appState":{},"files":{}}"#,
    )
    .expect("omission vector is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    let (result, trace) = render_with_trace(profile, &scene, &request);
    result.expect("diagnosed omission retains a meaningful artifact");
    let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
    let omissions = trace["entries"]
        .as_array()
        .expect("trace entries are an array")
        .iter()
        .filter(|entry| entry["event"] == "compatibility_omission")
        .collect::<Vec<_>>();
    let [omission] = omissions.as_slice() else {
        panic!("one omission transaction is retained");
    };
    assert_eq!(omission["json_pointer"], "/elements/0");
    assert_eq!(omission["deltas"]["total_work_units"], 8);
}

fn test_diagnostic() -> CompatibilityDiagnostic {
    CompatibilityDiagnostic {
        code: DiagnosticCode::COLOR_INVALID,
        severity: DiagnosticSeverity::Warning,
        format: OutputFormat::Svg,
        effects: vec![DiagnosticEffect::Visual],
        element_id: Some("element-1".to_owned()),
        json_pointer: "/elements/0/color".to_owned(),
        message: "invalid color substituted".to_owned(),
        details: None,
        order: DiagnosticOrder::default(),
    }
}

#[test]
fn png_raster_distinguishes_every_stroke_style() {
    // tiny-skia treats `Stroke::dash` as inert for the low-level stroker: the
    // path must be dashed before stroking. Skipping that step rendered dashed
    // and dotted elements as solid in PNG while SVG and PDF emitted the correct
    // dash arrays, so this pins all three styles apart in the raster.
    let render_one = |stroke_style: &str| {
        let scene = SceneDocument::parse(
            format!(
                r##"{{"type":"excalidraw","version":2,"elements":[{{"id":"probe","type":"ellipse","x":20,"y":20,"width":200,"height":120,"angle":0,"strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":2,"strokeStyle":"{stroke_style}","roughness":0,"opacity":100,"groupIds":[],"frameId":null,"index":"a1","roundness":null,"seed":4242,"version":1,"versionNonce":4243,"isDeleted":false,"boundElements":null,"updated":1,"link":null,"locked":false}}],"appState":{{"viewBackgroundColor":"#ffffff"}},"files":{{}}}}"##
            )
            .as_bytes(),
        )
        .expect("stroke-style probe scene is admitted");
        let request = RenderRequest::builder(OutputFormat::Png)
            .build()
            .expect("PNG request is valid");
        let artifact = Renderer::new()
            .render(&scene, &request)
            .expect("stroke-style probe renders");
        Sha256::digest(artifact.artifact().bytes()).to_vec()
    };

    let solid = render_one("solid");
    let dashed = render_one("dashed");
    let dotted = render_one("dotted");

    assert_ne!(solid, dashed, "dashed strokes must not rasterize as solid");
    assert_ne!(solid, dotted, "dotted strokes must not rasterize as solid");
    assert_ne!(
        dashed, dotted,
        "dashed and dotted strokes use different dash arrays and must differ"
    );
}

#[test]
fn bundled_faces_carry_their_families_metadata_metrics_and_only_helvetica_differs() {
    // Text is positioned by the source family's metadata metrics (#105). For
    // every bundled family those equal the face's own hhea values, so moving
    // the baseline from the face to the table changes no bundled family's
    // output; Helvetica is shaped with Liberation Sans, whose metrics differ.
    let source_family = |catalog_family: &str| match catalog_family {
        "Virgil" => 1,
        "Cascadia" => 3,
        "Excalifont" => 5,
        "Nunito" => 6,
        "Lilita" => 7,
        "ComicShanns" => 8,
        "Liberation" => 9,
        "Xiaolai" => 100,
        other => panic!("unexpected bundled font family {other}"),
    };
    let mut checked = 0_usize;
    for record in excalidraw_render_assets_base::revision_1()
        .assets
        .iter()
        .filter(|record| record.role == "official-rendering-font")
    {
        let asset = excalidraw_render_assets_base::load_asset(record.upstream_path)
            .expect("bundled font is intact")
            .expect("bundled font is present");
        let sfnt = wuff::decompress_woff2(asset.bytes()).expect("bundled font decodes");
        let face = rustybuzz::Face::from_slice(&sfnt, 0).expect("bundled font parses");
        let metadata = FamilyMetrics::for_source_family(source_family(record.family))
            .expect("every bundled family has metadata metrics");
        assert_eq!(
            (
                i32::from(metadata.units_per_em),
                metadata.ascender,
                metadata.descender
            ),
            (face.units_per_em(), face.ascender(), face.descender()),
            "{} hhea table differs from its family metadata metrics",
            record.upstream_path
        );
        checked += 1;
    }
    assert!(checked >= 8, "every bundled family was checked ({checked})");

    let helvetica = FamilyMetrics::for_source_family(2).expect("Helvetica has metrics");
    let liberation = FamilyMetrics::for_source_family(9).expect("Liberation has metrics");
    assert_eq!(
        (
            helvetica.units_per_em,
            helvetica.ascender,
            helvetica.descender
        ),
        (2_048, 1_577, -471)
    );
    assert_ne!(helvetica, liberation);
}
