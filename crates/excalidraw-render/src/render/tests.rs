use super::geometry::{adjusted_roughness, rough_drawing};
use super::resolve::{
    ResolvedImageCornerClip, ResolvedImageCropPlacement, ResolvedImageProjection,
};
use super::*;
use crate::normalization::{NormalizedElement, NormalizedElementKind};
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

/// The trace is held stable as the equalities below (repeated traced renders
/// agree, and a traced render leaves the artifact and report untouched) and by
/// the cross-target aggregate, not as pinned digests (ADR-0002).
#[test]
fn canonical_resource_trace_is_uncharged_stable_and_semantic() {
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
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
    let selects = |text: &str| text::CjkSubsetTrigger::evaluate([text]).selects_xiaolai();
    assert!(selects("\u{4e2d}"));
    assert!(selects("\u{d7b0}"));
    assert!(selects("￥"));
    assert!(!selects("\u{2ff0}"));
    assert!(!selects("A"));
    assert!(!selects(""));
    // The trigger is a scalar-set intersection over every occurrence, not a
    // per-occurrence property.
    assert!(text::CjkSubsetTrigger::evaluate(["A", "\u{4e2d}"]).selects_xiaolai());
}

#[test]
fn cascadia_calt_ligatures_lay_out_through_their_empty_spacer_glyphs() {
    // Cascadia Code spells `->`, `??`, `www`, and the rest of its ligature set
    // through `calt`: every character but the last becomes the empty `LIG`
    // spacer glyph, and the last carries the joined outline. An empty outline
    // in a reviewed chunk is font content, not corruption, so the text lays
    // out and renders. Found by the scene fuzz as `fontFamily: 3` with two
    // CJK scalars, whose `??` replacement glyphs form the same ligature.
    for text_value in ["->", "??", "www", "a != b", "\u{4e2d}\u{6587}"] {
        let font = text::OfficialFont::load(3, text_value)
            .ok()
            .unwrap_or_else(|| panic!("Cascadia Code loads for {text_value:?}"));
        let laid_out = font
            .layout(
                text_value,
                text::TextLayout {
                    font_family: 3,
                    ..excalifont_layout()
                },
            )
            .ok()
            .unwrap_or_else(|| panic!("{text_value:?} lays out in Cascadia Code"));
        let glyphs = laid_out
            .lines
            .iter()
            .map(|line| line.glyphs.len())
            .sum::<usize>();
        assert_eq!(
            glyphs,
            text_value.chars().count(),
            "{text_value:?} keeps one glyph slot per scalar"
        );

        let source = format!(
            r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"code","type":"text","width":120,"height":25,"fontSize":20,"fontFamily":3,"text":{},"updated":1}}],"appState":{{}},"files":{{}}}}"#,
            serde_json::to_string(text_value).expect("text serializes")
        );
        let scene = SceneDocument::parse(source.as_bytes()).expect("ligature scene is admitted");
        let expects_replacement = !text_value.is_ascii();
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format)
                .build()
                .expect("default request is valid");
            let outcome = Renderer::new()
                .render(&scene, &request)
                .unwrap_or_else(|error| panic!("{text_value:?} renders to {format:?}: {error}"));
            let replaced = outcome
                .report()
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == DiagnosticCode::FONT_MISSING_GLYPH);
            assert_eq!(
                replaced,
                expects_replacement,
                "{text_value:?} in {format:?} replacement glyphs: {:?}",
                outcome.report()
            );
        }
    }
}

/// Lays `text` out with fallbacks against a fresh font list whose only face
/// is the whole-text face `OfficialFont::load` selects for it, and returns the
/// font fallback probes it charged.
fn fallback_probes_for(font_family: i32, text: &str) -> u64 {
    let mut fonts = vec![text::OfficialFont::load(font_family, text)
        .ok()
        .unwrap_or_else(|| panic!("family {font_family} loads for {text:?}"))];
    fallback_probes_with(&mut fonts, font_family, text)
}

/// Lays `text` out with fallbacks against `fonts`, whose first face is the
/// whole-text face of `font_family`, and returns the font fallback probes it
/// charged.
fn fallback_probes_with(fonts: &mut Vec<text::OfficialFont>, font_family: i32, text: &str) -> u64 {
    let trigger = text::CjkSubsetTrigger::evaluate([text]);
    let layout = text::TextLayout {
        font_family,
        ..excalifont_layout()
    };
    let (laid_out, _) = text::layout_with_fallbacks(0, fonts, text, layout, trigger)
        .ok()
        .unwrap_or_else(|| panic!("{text:?} lays out with fallbacks"));
    laid_out.fallback_probes
}

#[test]
fn a_font_fallback_probe_is_one_consulted_face_per_shaped_scalar_occurrence() {
    // Spec 8.3.1: a shaped scalar the whole-text face maps pays no probe.
    // Otherwise it pays one probe for each official font chunk of its family
    // whose declared Unicode range covers it, in pinned face order; then, for
    // Excalifont when the CJK subset trigger selected Xiaolai, one for each
    // Xiaolai chunk likewise; then one for the Noto Emoji consultation; then
    // one for the Liberation Sans replacement glyph. Every occurrence pays its
    // own probes. Expected counts follow that rule, not the code.
    assert_eq!(
        fallback_probes_for(5, "A"),
        0,
        "the whole-text face maps it"
    );
    assert_eq!(
        fallback_probes_for(5, "\u{20a9}"),
        2,
        "no Excalifont chunk declares the won sign: Noto Emoji, then the replacement"
    );
    assert_eq!(
        fallback_probes_for(5, "\u{10ffff}"),
        2,
        "nothing declares it either: Noto Emoji, then the replacement"
    );
    assert_eq!(
        fallback_probes_for(5, "\u{4e2d}"),
        1,
        "triggered: the one Xiaolai chunk declaring the ideograph maps it"
    );
    assert_eq!(
        fallback_probes_for(5, "\u{1f600}"),
        1,
        "Noto Emoji maps the emoji on its one consultation"
    );
    assert_eq!(
        fallback_probes_for(5, "\u{4e2d}\u{20a9}"),
        2,
        "triggered: the won sign is served by the one Xiaolai chunk declaring it"
    );
    assert_eq!(
        fallback_probes_for(5, "\u{20a9}\u{20a9}"),
        4,
        "a repeated scalar pays its probes on every occurrence"
    );
    assert_eq!(
        fallback_probes_for(9, "\u{5d0}"),
        3,
        "Liberation declares every scalar but maps no Hebrew: its chunk, Noto Emoji, the replacement"
    );
}

#[test]
fn a_selected_noto_emoji_face_is_consulted_once_per_scalar_like_an_unselected_one() {
    // Spec 8.3.1: whether a consulted face already serves the target changes
    // neither the probe count nor the order. The Noto Emoji consultation is
    // one probe whether the face is being selected now, was selected by an
    // earlier scalar of the same occurrence, or by an earlier occurrence.
    assert_eq!(
        fallback_probes_for(5, "\u{1f600}\u{20a9}"),
        1 + 2,
        "the emoji selects Noto Emoji; the won sign then pays Noto Emoji and the replacement"
    );
    assert_eq!(
        fallback_probes_for(5, "\u{1f600}\u{1f601}"),
        1 + 1,
        "the second emoji pays the one consultation of the selected face"
    );
    let mut fonts = vec![text::OfficialFont::load(5, "\u{1f600}").expect("Excalifont loads")];
    assert_eq!(fallback_probes_with(&mut fonts, 5, "\u{1f600}"), 1);
    assert_eq!(
        fallback_probes_with(&mut fonts, 5, "\u{20a9}"),
        2,
        "a later occurrence pays the won sign what a first occurrence would"
    );
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
    let font = text::FallbackFont::load().expect("the bundled Liberation Sans loads");
    let baseline = FamilyMetrics::for_source_family(2)
        .expect("Helvetica has metadata metrics")
        .first_baseline(14.0, 17.5);
    assert!((baseline - 12.530_273_437_5).abs() < 1e-12);

    // A name narrower than its frame keeps its measured width and starts at
    // the frame's x.
    let name = font
        .resolve_frame_name("Clip", 580.0)
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
        .expect("the full name shapes");
    let narrow = font
        .resolve_frame_name("A rather long frame name", full.width / 2.0)
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
        .expect("the tiny frame's name shapes");
    assert_eq!(tiny.label.text, "Clip");
    assert_eq!(tiny.width, 1.0);

    // A blank name measures as one space, as measureText does.
    let blank = font
        .resolve_frame_name("", 100.0)
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
    let font = text::OfficialFont::load(5, "a\r\nb").expect("Excalifont loads");
    let laid_out = font
        .layout("a\r\nb", excalifont_layout())
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
    let font = text::OfficialFont::load(5, "ab").expect("Excalifont loads");
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
    let mut fonts = vec![text::OfficialFont::load(5, "ab").expect("Excalifont loads")];
    for (source, expected) in [
        ("a\r\nb", vec![("a\r\n", "a"), ("b", "b")]),
        ("a\rb\r", vec![("a\r", "a"), ("b\r", "b"), ("", "")]),
        // CJK is not in Excalifont, so each line here is a Xiaolai fallback run.
        ("中\r\n文", vec![("中\r\n", "中"), ("文", "文")]),
    ] {
        let trigger = text::CjkSubsetTrigger::evaluate([source]);
        let (laid_out, _) =
            text::layout_with_fallbacks(0, &mut fonts, source, excalifont_layout(), trigger)
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

/// The vertical family metadata metrics Excalidraw's `FONT_METADATA` declares
/// and `getVerticalOffset` reads to place a first baseline: the independent
/// source the baseline assertions below are worked from.
struct Metrics {
    units_per_em: f64,
    ascender: f64,
    descender: f64,
}

impl Metrics {
    /// `font_size * (line_height / 2 + (ascender + descender) / units_per_em / 2)`.
    fn first_baseline(&self, font_size: f64, line_height: f64) -> f64 {
        font_size * (line_height / 2.0 + (self.ascender + self.descender) / self.units_per_em / 2.0)
    }
}

const HELVETICA: Metrics = Metrics {
    units_per_em: 2048.0,
    ascender: 1577.0,
    descender: -471.0,
};
const LIBERATION_SANS: Metrics = Metrics {
    units_per_em: 2048.0,
    ascender: 1854.0,
    descender: -434.0,
};
const CASCADIA: Metrics = Metrics {
    units_per_em: 2048.0,
    ascender: 1900.0,
    descender: -480.0,
};
const NUNITO: Metrics = Metrics {
    units_per_em: 1000.0,
    ascender: 1011.0,
    descender: -353.0,
};
const EXCALIFONT: Metrics = Metrics {
    units_per_em: 1000.0,
    ascender: 886.0,
    descender: -374.0,
};

/// Asserts the laid-out lines sit at `expected` baselines, in order.
fn assert_baselines(laid_out: &text::ResolvedText, expected: &[f64]) {
    let actual = laid_out
        .lines
        .iter()
        .map(|line| line.baseline)
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len(), "line count: {actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "baseline {actual} should be {expected}"
        );
    }
}

/// Lays `text` out in the font `OfficialFont::load` selects for `loaded_family`,
/// positioned by `layout_family`'s metadata metrics.
fn layout_in_family(
    loaded_family: i32,
    layout_family: i32,
    text: &str,
    font_size: f64,
    line_height: f64,
) -> text::ResolvedText {
    text::OfficialFont::load(loaded_family, text)
        .ok()
        .unwrap_or_else(|| panic!("family {loaded_family} loads"))
        .layout(
            text,
            text::TextLayout {
                width: 200.0,
                font_size,
                line_height,
                text_align: "center",
                font_family: layout_family,
            },
        )
        .ok()
        .unwrap_or_else(|| panic!("family {layout_family} lays out"))
}

#[test]
fn helvetica_text_is_positioned_by_excalidraw_metadata_metrics_not_the_shaping_font() {
    // Helvetica (family 2) is shaped with the bundled Liberation Sans, but
    // `getVerticalOffset` reads Helvetica's own `FONT_METADATA` metrics, so
    // its baselines differ from Liberation Sans text (family 9) at the same
    // size although one font shapes both (#105). Whichever family loaded the
    // shared font, the layout's own family decides the baseline.
    // `1.15 / 2 + (1577 - 471) / 2048 / 2` and `1.15 / 2 + (1854 - 434) / 2048 / 2`:
    // the first-baseline factors at line height 1.15 for each family.
    let helvetica_first = HELVETICA.first_baseline(28.0, 1.15);
    assert_eq!(helvetica_first, 28.0 * 0.84501953125);
    let liberation_first = LIBERATION_SANS.first_baseline(28.0, 1.15);
    assert_eq!(liberation_first, 28.0 * 0.9216796875);
    let line_step = 28.0 * 1.15;
    for loaded_family in [2, 9] {
        let helvetica = layout_in_family(loaded_family, 2, "Two lines\nof Helvetica", 28.0, 1.15);
        assert_baselines(&helvetica, &[helvetica_first, helvetica_first + line_step]);
        assert_eq!(helvetica.font_family, "Liberation Sans");

        let liberation = layout_in_family(loaded_family, 9, "Two lines\nof Helvetica", 28.0, 1.15);
        assert_baselines(
            &liberation,
            &[liberation_first, liberation_first + line_step],
        );
        assert_eq!(liberation.font_family, "Liberation Sans");
    }
}

#[test]
fn the_first_baseline_follows_the_family_metadata_metrics_at_any_line_height() {
    // The layout positions the first baseline from the source family's
    // metadata metrics and the line height it is given, whether that is the
    // element's own or the one normalization restored for it (#108; the
    // restore itself is tested in `normalization::tests`, and that a restored
    // line height reaches the render in `legacy_font_strings_render_as_the_
    // official_font_size_and_family_migration_in_every_format`). The values
    // here are what the restore detects, `height / lines / fontSize`, for the
    // first three and the family metadata line height for the rest.
    for (family, text, font_size, line_height, metrics) in [
        (2, "First", 20.0, 1.5, &HELVETICA),
        (3, "Code", 20.0, 1.5, &CASCADIA),
        (2, "Two\nlines", 12.0, 2.0, &HELVETICA),
        (6, "Nunito", 20.0, 1.35, &NUNITO),
        (5, "Control", 20.0, 1.25, &EXCALIFONT),
        (2, "Helvetica", 20.0, 1.15, &HELVETICA),
        (9, "Liberation", 20.0, 1.15, &LIBERATION_SANS),
    ] {
        let laid_out = layout_in_family(family, family, text, font_size, line_height);
        let first = metrics.first_baseline(font_size, line_height);
        let expected = (0..laid_out.lines.len())
            .map(|index| first + index as f64 * font_size * line_height)
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), text.split('\n').count(), "{text:?}");
        assert_baselines(&laid_out, &expected);
    }
}

#[test]
fn crlf_and_cr_lines_lay_out_as_lf_lines_do() {
    // The reference renders `text.replace(/\r\n?/g, "\n").split("\n")` and
    // leaves `text` alone: the laid-out lines, their glyphs and their
    // baselines are the same for every line ending, and only the per-line
    // source text, which SVG carries in `data-source-text`, keeps the
    // element's own bytes (#110).
    let font = text::OfficialFont::load(5, "ab").expect("Excalifont loads");
    let lf = font
        .layout("a\nb", excalifont_layout())
        .expect("LF text lays out");
    let first = EXCALIFONT.first_baseline(20.0, 1.25);
    assert_baselines(&lf, &[first, first + 25.0]);
    let display_glyphs = |laid_out: &text::ResolvedText| {
        laid_out
            .lines
            .iter()
            .map(|line| {
                (
                    line.display_text.clone(),
                    line.glyphs
                        .iter()
                        .map(|glyph| (glyph.id, glyph.x))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    for (source, sources) in [("a\r\nb", ["a\r\n", "b"]), ("a\rb", ["a\r", "b"])] {
        let other = font
            .layout(source, excalifont_layout())
            .ok()
            .unwrap_or_else(|| panic!("{source:?} lays out"));
        assert_baselines(&other, &[first, first + 25.0]);
        assert_eq!(display_glyphs(&other), display_glyphs(&lf), "{source:?}");
        assert_eq!(line_texts(&other), [(sources[0], "a"), (sources[1], "b")]);
        assert!(other.missing_codepoints.is_empty(), "{source:?}");
    }
}

/// Lays `source` out with fallbacks for `font_family`, against a fresh font
/// list whose only face is the whole-text face `OfficialFont::load` selects.
fn fallback_layout(font_family: i32, source: &str) -> text::ResolvedText {
    let mut fonts = vec![text::OfficialFont::load(font_family, source)
        .ok()
        .unwrap_or_else(|| panic!("family {font_family} loads for {source:?}"))];
    let layout = text::TextLayout {
        font_family,
        ..excalifont_layout()
    };
    let trigger = text::CjkSubsetTrigger::evaluate([source]);
    text::layout_with_fallbacks(0, &mut fonts, source, layout, trigger)
        .ok()
        .unwrap_or_else(|| panic!("{source:?} lays out with fallbacks"))
        .0
}

/// The one line of `source` laid out with fallbacks for `font_family`.
fn fallback_line(font_family: i32, source: &str) -> text::ResolvedLabel {
    let mut laid_out = fallback_layout(font_family, source);
    assert_eq!(laid_out.lines.len(), 1, "{source:?} is one line");
    laid_out.lines.remove(0)
}

/// `(display text, rtl)` per glyph run of the one line `laid_out` has.
fn single_line_runs(laid_out: &text::ResolvedText) -> Vec<(&str, bool)> {
    let [line] = &laid_out.lines[..] else {
        panic!("one source line, got {}", laid_out.lines.len());
    };
    runs_of(line)
}

/// `(display text, rtl)` per glyph run of `line`.
fn runs_of(line: &text::ResolvedLabel) -> Vec<(&str, bool)> {
    line.runs
        .iter()
        .map(|run| (run.display_text.as_str(), run.rtl))
        .collect()
}

/// `(display text, rtl, face family)` per glyph run of `line`.
fn faced_runs_of(line: &text::ResolvedLabel) -> Vec<(&str, bool, &str)> {
    line.runs
        .iter()
        .map(|run| (run.display_text.as_str(), run.rtl, run.font_family))
        .collect()
}

/// The glyphs of the run at `index`, in the order the line places them, as
/// `(logical cluster text, glyph id, x)`.
fn run_glyphs(line: &text::ResolvedLabel, index: usize) -> Vec<(String, u16, f64)> {
    let run = &line.runs[index];
    line.glyphs[run.glyph_start..run.glyph_end]
        .iter()
        .map(|glyph| {
            (
                String::from_utf16(&glyph.unicode).expect("cluster text is UTF-16"),
                glyph.id,
                glyph.x,
            )
        })
        .collect()
}

/// The glyph id each scalar of the run at `index` is positioned by in SVG, in
/// logical order.
fn logical_glyph_ids(line: &text::ResolvedLabel, index: usize) -> Vec<u16> {
    line.runs[index]
        .logical_glyphs
        .iter()
        .map(|glyph| line.glyphs[*glyph].id)
        .collect()
}

/// The glyph ids `face` maps `characters` to.
fn glyph_ids(font_family: i32, characters: &str) -> Vec<u16> {
    let (_, _, _, _, _, sfnt) = text::OfficialFont::load(font_family, characters)
        .ok()
        .unwrap_or_else(|| panic!("family {font_family} loads for {characters:?}"))
        .into_parts();
    let face = rustybuzz::Face::from_slice(&sfnt, 0).expect("face parses");
    characters
        .chars()
        .map(|character| {
            face.glyph_index(character)
                .unwrap_or_else(|| panic!("family {font_family} maps {character:?}"))
                .0
        })
        .collect()
}

#[test]
fn every_bidi_paragraph_of_a_source_line_is_laid_out() {
    // Spec 8.3 pins the reference's line rule (`\r\n`, `\r`, `\n` only), so a
    // UAX #9 paragraph separator stays inside one source line while
    // unicode-bidi splits the line into paragraphs at it (P1). Every paragraph
    // is content: each is laid out in order, and the separator itself, which
    // no bundled face maps, is the replacement glyph between them (#125).
    for separator in ['\u{2029}', '\u{85}', '\u{1c}', '\u{1d}', '\u{1e}'] {
        let source = format!("ab{separator}cd");
        let laid_out = fallback_layout(5, &source);
        assert_eq!(
            line_texts(&laid_out),
            [(source.as_str(), "ab?cd")],
            "{source:?} is one source line whose every paragraph is shown"
        );
        assert_eq!(
            single_line_runs(&laid_out),
            [("ab", false), ("?", false), ("cd", false)],
            "{source:?} lays its paragraphs out in order"
        );
        assert_eq!(
            laid_out.missing_codepoints,
            [u32::from(separator)],
            "{source:?} reports only the separator"
        );
        assert_eq!(laid_out.glyph_count(), 5, "{source:?} shapes every scalar");
        assert_eq!(
            laid_out.glyph_count(),
            laid_out.prepaid_glyphs,
            "{source:?}"
        );
    }

    // Each paragraph detects its own base level from its own first strong
    // character: Hebrew then a separator is one right-to-left paragraph, the
    // Latin after it a left-to-right one.
    let source = "\u{5d0}\u{2029}ab";
    let laid_out = fallback_layout(9, source);
    assert_eq!(line_texts(&laid_out), [(source, "??ab")]);
    assert_eq!(single_line_runs(&laid_out), [("??", true), ("ab", false)]);
    assert_eq!(laid_out.missing_codepoints, [0x5d0, 0x2029]);
}

#[test]
fn a_right_to_left_run_is_shaped_right_to_left() {
    // Spec 8.3: within a run, glyph order and placement are the shaper's for
    // the run's display text in the run's direction. No bundled face maps
    // Hebrew, so an RTL run is replacement glyphs plus neutrals; guessing the
    // direction from that text would shape it left to right and leave an
    // asymmetric neutral at the run's logical end instead of its visual end
    // (#126). UAX #9 puts the space of "אבג " and the period of "אב." at the
    // left edge of their runs.
    let line = fallback_line(9, "abc \u{5d0}\u{5d1}\u{5d2} 123");
    assert_eq!(
        runs_of(&line),
        [("abc ", false), ("123", false), ("??? ", true)]
    );
    let hebrew = run_glyphs(&line, 2);
    assert_eq!(
        hebrew
            .iter()
            .map(|(text, ..)| text.as_str())
            .collect::<Vec<_>>(),
        [" ", "?", "?", "?"],
        "the space is the run's leftmost glyph"
    );
    assert!(
        hebrew.windows(2).all(|pair| pair[0].2 < pair[1].2),
        "glyphs are placed left to right in visual order: {hebrew:?}"
    );
    let digits = run_glyphs(&line, 1);
    let digits_right_edge = digits
        .last()
        .map(|(_, _, x)| *x)
        .expect("digits have glyphs");
    assert!(
        hebrew[0].2 > digits_right_edge,
        "the Hebrew run starts after the digits' last glyph origin"
    );

    let line = fallback_line(9, "\u{5d0}\u{5d1}.");
    assert_eq!(runs_of(&line), [("??.", true)]);
    assert_eq!(
        run_glyphs(&line, 0)
            .iter()
            .map(|(text, ..)| text.as_str())
            .collect::<Vec<_>>(),
        [".", "?", "?"],
        "the period is the run's leftmost glyph"
    );

    // UAX #9 L4: a mirrored character in a right-to-left run shows its mirror
    // glyph, so "(א)" still reads as an opening paren, the text, a closing
    // paren from left to right. The cluster text each glyph carries stays the
    // logical character.
    let [opening, closing] = glyph_ids(9, "()")[..] else {
        panic!("two glyphs");
    };
    let line = fallback_line(9, "(\u{5d0})");
    assert_eq!(runs_of(&line), [("(?)", true)]);
    let parens = run_glyphs(&line, 0);
    assert_eq!(
        parens
            .iter()
            .map(|(text, id, _)| (text.as_str(), *id))
            .collect::<Vec<_>>(),
        [(")", opening), ("?", parens[1].1), ("(", closing)],
        "the logical closing paren shows an opening glyph at the left, and vice versa"
    );
}

#[test]
fn face_sub_runs_of_a_right_to_left_run_are_placed_right_to_left() {
    // Spec 8.3: a visual run is split further by the face that shapes its
    // scalars, and every glyph run is laid out left to right in visual order.
    // Excalifont shapes the period of "אב." while Liberation Sans shapes the
    // replacement glyphs, so the one right-to-left visual run is two glyph
    // runs; the period is logically last, hence visually leftmost (#126).
    let line = fallback_line(5, "\u{5d0}\u{5d1}.");
    assert_eq!(
        faced_runs_of(&line),
        [(".", true, "Excalifont"), ("??", true, "Liberation Sans")],
        "the face runs of a right-to-left visual run follow in visual order"
    );
    let period = run_glyphs(&line, 0);
    let replacements = run_glyphs(&line, 1);
    assert!(
        period[0].2 < replacements[0].2,
        "the period sits left of the replacement glyphs: {period:?} vs {replacements:?}"
    );
    assert!(
        period[0].2.abs() < 1e-9,
        "left-aligned text starts at the line origin"
    );
    assert_eq!(
        line.display_text, ".??",
        "the line's display text follows its glyph runs"
    );
}

#[test]
fn every_scalar_of_a_run_is_positioned_by_its_own_glyph_in_either_direction() {
    // SVG assigns the nth x/y of a run's tspan to its nth logical character.
    // The shaper merges a combining mark into its base's cluster, and a
    // right-to-left run comes back reversed, marks included; each scalar is
    // still positioned by the glyph that shows it, so the mark keeps the
    // offset the shaper gave it over the base rather than the base's origin.
    // Nunito's Latin chunk maps the combining dot below and keeps it a glyph
    // of its own; Liberation's chunk maps no combining mark.
    let [e, question, dot_below] = glyph_ids(6, "e?\u{323}")[..] else {
        panic!("three glyphs");
    };

    // Left to right: "e" with a dot below, forced onto the fallback path by
    // Hebrew.
    let line = fallback_line(6, "e\u{323}\u{5d0}");
    assert_eq!(runs_of(&line), [("e\u{323}", false), ("?", true)]);
    assert_eq!(logical_glyph_ids(&line, 0), [e, dot_below]);
    let [base, mark] = line.runs[0].logical_glyphs[..] else {
        panic!("two scalars");
    };
    assert_ne!(base, mark, "each scalar is positioned by its own glyph");
    assert_eq!(
        line.glyphs[base].cluster, line.glyphs[mark].cluster,
        "the shaper merged the mark into its base's cluster"
    );
    assert_ne!(
        line.glyphs[base].x, line.glyphs[mark].x,
        "the mark keeps the offset the shaper gave it over the base"
    );

    // Right to left: a dot below a question mark inside a Hebrew paragraph.
    // The Hebrew is a Liberation Sans replacement glyph, the rest Nunito's,
    // so the visual run is two glyph runs placed right to left.
    let line = fallback_line(6, "\u{5d0}?\u{323}");
    assert_eq!(
        faced_runs_of(&line),
        [("?\u{323}", true, "Nunito"), ("?", true, "Liberation Sans")]
    );
    assert_eq!(logical_glyph_ids(&line, 0), [question, dot_below]);
    let mut positioned = line.runs[0].logical_glyphs.clone();
    positioned.sort_unstable();
    assert_eq!(
        positioned,
        (line.runs[0].glyph_start..line.runs[0].glyph_end).collect::<Vec<_>>(),
        "every glyph positions exactly one scalar"
    );

    // When the shaper composes base and mark into one glyph (Nunito's `ccmp`
    // for the acute), both scalars are positioned by that glyph.
    let [e_acute] = glyph_ids(6, "\u{e9}")[..] else {
        panic!("one glyph");
    };
    let line = fallback_line(6, "e\u{301}\u{5d0}");
    assert_eq!(runs_of(&line), [("e\u{301}", false), ("?", true)]);
    assert_eq!(logical_glyph_ids(&line, 0), [e_acute, e_acute]);
    assert_eq!(line.runs[0].glyph_end - line.runs[0].glyph_start, 1);
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

#[test]
fn an_image_corner_radius_follows_get_corner_radius_of_its_shorter_side() {
    use crate::normalization::NormalizedRoundness;
    // `getCornerRadius(Math.min(element.width, element.height), element)`:
    // ROUNDNESS.LEGACY and PROPORTIONAL_RADIUS give a quarter of the size;
    // ADAPTIVE_RADIUS gives its fixed value until the size drops under four
    // times that value, where the proportional radius takes over.
    assert_eq!(
        geometry::image_corner_radius(100.0, 40.0, Some(NormalizedRoundness::Proportional)),
        10.0
    );
    assert_eq!(
        geometry::image_corner_radius(40.0, 100.0, Some(NormalizedRoundness::Proportional)),
        10.0
    );
    let default_adaptive = Some(NormalizedRoundness::Adaptive { radius: 32.0 });
    assert_eq!(
        geometry::image_corner_radius(200.0, 300.0, default_adaptive),
        32.0
    );
    assert_eq!(
        geometry::image_corner_radius(128.0, 300.0, default_adaptive),
        32.0
    );
    assert_eq!(
        geometry::image_corner_radius(100.0, 300.0, default_adaptive),
        25.0
    );
    assert_eq!(
        geometry::image_corner_radius(
            100.0,
            40.0,
            Some(NormalizedRoundness::Adaptive { radius: 8.0 })
        ),
        8.0
    );
    assert_eq!(geometry::image_corner_radius(100.0, 40.0, None), 0.0);
}

#[test]
fn an_image_corner_clip_exists_only_for_a_positive_radius_and_traces_its_rounded_rectangle() {
    use crate::normalization::NormalizedRoundness;

    let Ok(Some(clip)) =
        ResolvedImageCornerClip::for_image(40.0, 20.0, Some(NormalizedRoundness::Proportional))
    else {
        panic!("a rounded image has a corner clip");
    };
    assert_eq!(clip.radius, 5.0);
    // The same nine-operation outline as the frame clip: four edges joined by
    // four kappa-approximated quarter arcs, starting and ending at the top
    // edge's left arc end.
    assert_eq!(clip.path.operations.len(), 9);
    let corners: Vec<(f64, f64)> = clip
        .path
        .operations
        .iter()
        .filter_map(|operation| match operation {
            geometry::PathOperation::Move(point) | geometry::PathOperation::Line(point) => {
                Some((point.x, point.y))
            }
            geometry::PathOperation::Cubic { .. } => None,
        })
        .collect();
    assert_eq!(
        corners,
        [
            (5.0, 0.0),
            (35.0, 0.0),
            (40.0, 15.0),
            (5.0, 20.0),
            (0.0, 5.0)
        ]
    );
    assert!(matches!(
        clip.path.operations.last(),
        Some(geometry::PathOperation::Cubic { end, .. }) if end.x == 5.0 && end.y == 0.0
    ));

    assert!(
        matches!(
            ResolvedImageCornerClip::for_image(40.0, 20.0, None),
            Ok(None)
        ),
        "an unrounded image has no corner clip"
    );
    assert!(
        matches!(
            ResolvedImageCornerClip::for_image(
                40.0,
                20.0,
                Some(NormalizedRoundness::Adaptive { radius: 0.0 })
            ),
            Ok(None)
        ),
        "a zero radius clips nothing the rectangle does not already clip"
    );
    assert!(
        matches!(
            ResolvedImageCornerClip::for_image(0.0, 20.0, Some(NormalizedRoundness::Proportional)),
            Ok(None)
        ),
        "a degenerate element has no corner to round"
    );
}

#[test]
fn an_image_projection_rounds_only_its_viewport_the_way_the_reference_use_does() {
    use crate::normalization::NormalizedImageCrop;

    // The reference `<use>` is `Math.round(width)` by `Math.round(height)`,
    // and an uncropped image fills it as the reference's 100% symbol image does.
    let projection = ResolvedImageProjection::for_image(40.5, 40.4, None);
    assert_eq!(projection.viewport_width, 41.0);
    assert_eq!(projection.viewport_height, 40.0);
    assert_eq!(projection.crop, None);
    assert_eq!(projection.content_rect(), (0.0, 0.0, 41.0, 40.0));

    // `Math.round` ties go up, not away from zero as `f64::round` would.
    let ties = ResolvedImageProjection::for_image(2.5, 0.5, None);
    assert_eq!(ties.viewport_width, 3.0);
    assert_eq!(ties.viewport_height, 1.0);
    let integer = ResolvedImageProjection::for_image(96.0, 40.0, None);
    assert_eq!(integer.viewport_width, 96.0);
    assert_eq!(integer.viewport_height, 40.0);
    let just_under = ResolvedImageProjection::for_image(0.499_999_999_999_999_94, 1.0, None);
    assert_eq!(just_under.viewport_width, 0.0);

    // A cropped image keeps the reference's unrounded uncropped size and crop
    // offset: `getUncroppedWidthAndHeight` and `normalizedCropX/Y` from the
    // oracle, for a 3x2 source cropped to its right 2x2 in a 20.5x20.4 element.
    let crop = NormalizedImageCrop {
        x: 1.0,
        y: 0.5,
        width: 2.0,
        height: 2.0,
        natural_width: 3.0,
        natural_height: 2.0,
    };
    let projection = ResolvedImageProjection::for_image(20.5, 20.4, Some(crop));
    assert_eq!(projection.viewport_width, 21.0);
    assert_eq!(projection.viewport_height, 20.0);
    assert_eq!(
        projection.crop,
        Some(ResolvedImageCropPlacement {
            offset_x: 10.25,
            offset_y: 5.1,
            uncropped_width: 30.75,
            uncropped_height: 20.4,
        })
    );
    assert_eq!(projection.content_rect(), (-10.25, -5.1, 30.75, 20.4));
}

#[test]
fn a_rounded_image_charges_its_corner_clip_geometry_once_per_target_and_replays() {
    // A 40x40 image with the default adaptive roundness: a 10 px corner clip
    // of nine path operations, charged like a frame clip's geometry in the
    // semantic phase and identically for every format.
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[{
            "id":"photo","type":"image","x":0,"y":0,"width":40,"height":40,
            "angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],
            "roundness":{"type":3},"seed":1,"updated":1
        }],
        "appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=","created":1}}
    }"##)
    .expect("rounded image scene is admitted");
    let profile =
        Renderer::profile_identity(CompatibilityRevision::REVISION_1).expect("revision 1 exists");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("request is valid");
        let cache = new_font_subset_cache();
        let (cold_result, cold_trace) =
            render_with_trace_and_font_subset_cache(profile, &scene, &request, &cache);
        let cold = cold_result.expect("cold rounded image render succeeds");
        let (warm_result, warm_trace) = render_with_trace(profile, &scene, &request);
        let warm = warm_result.expect("warm rounded image render succeeds");
        let (cached_result, cached_trace) =
            render_with_trace_and_font_subset_cache(profile, &scene, &request, &cache);
        let cached = cached_result.expect("request-cache rounded image render succeeds");

        assert_eq!(
            cold.artifact().bytes(),
            warm.artifact().bytes(),
            "{format:?}"
        );
        assert_eq!(
            cold.artifact().bytes(),
            cached.artifact().bytes(),
            "{format:?}"
        );
        assert_eq!(cold_trace, warm_trace, "{format:?}");
        assert_eq!(cold_trace, cached_trace, "{format:?}");

        let trace: serde_json::Value =
            serde_json::from_slice(&cold_trace).expect("trace is valid JSON");
        let entries = trace["entries"].as_array().expect("entries are an array");
        let clips: Vec<&serde_json::Value> = entries
            .iter()
            .filter(|entry| entry["event"] == "image_corner_clip_geometry")
            .collect();
        assert_eq!(clips.len(), 1, "{format:?} charges the corner clip once");
        let clip = clips[0];
        assert_eq!(clip["phase"], "shared_semantic_resolution", "{format:?}");
        assert_eq!(clip["element_id"], "photo", "{format:?}");
        assert_eq!(clip["local_owner"], "photo", "{format:?}");
        assert_eq!(clip["json_pointer"], "/elements/0", "{format:?}");
        assert_eq!(clip["deltas"]["geometry_segments"], 9, "{format:?}");
        assert_eq!(clip["deltas"]["total_work_units"], 9 * 32, "{format:?}");
        assert_eq!(clip["decision"], "committed", "{format:?}");
    }

    let unrounded = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[{
            "id":"photo","type":"image","x":0,"y":0,"width":40,"height":40,
            "angle":0,"opacity":100,"fileId":"asset","status":"saved","scale":[1,1],
            "seed":1,"updated":1
        }],
        "appState":{},
        "files":{"asset":{"id":"asset","mimeType":"image/png","dataURL":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=","created":1}}
    }"##)
    .expect("unrounded image scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("request is valid");
    let (result, trace) = render_with_trace(profile, &unrounded, &request);
    result.expect("unrounded image renders");
    assert!(
        !std::str::from_utf8(&trace)
            .expect("trace is UTF-8 JSON")
            .contains("image_corner_clip_geometry"),
        "an unrounded image has no corner clip to charge"
    );
}

// Pinned Rough.js and perfect-freehand path vectors, recomputed from the
// geometry stage alone: normalization, then `rough_drawing` or
// `freedraw::generate`, then the SVG path writers. The digests are the ones
// the SVG renders of these scenes carried when they were pinned, so a change
// here is a change in the generated geometry, not in the document around it.

/// The normalized elements of `source` for a default SVG render.
fn normalized_elements(source: &[u8]) -> Vec<NormalizedElement> {
    let scene = SceneDocument::parse(source).expect("geometry scene is admitted");
    crate::normalization::normalize_scene(
        &scene,
        CompatibilityRevision::REVISION_1,
        OutputFormat::Svg,
        |_| Ok::<_, ()>(()),
    )
    .expect("normalization succeeds")
    .expect("the scene is in the implemented normalization slice")
    .elements
}

/// The default request's generation budget for `source`: the ceilings a
/// default render applies, which none of these scenes approach.
fn default_budget(source: &NormalizedElement) -> rough::GenerationBudget {
    let limits = ResourceLimits::default();
    rough::GenerationBudget::new(
        u64::from(limits.max_geometry_segments_per_element()),
        limits.max_element_work_units(),
        source
            .normalization_work_units
            .saturating_add(8)
            .saturating_add(source.repair_work_units),
    )
}

/// The SVG path data a default render writes for one Rough element, in
/// document order: the fill's path when the element has a background, then the
/// outline. Arrow decorations are not part of these pins.
fn rough_svg_paths(source: &NormalizedElement) -> Vec<String> {
    let drawing = rough_drawing(
        source,
        adjusted_roughness(source),
        &mut default_budget(source),
    )
    .ok()
    .unwrap_or_else(|| panic!("{} generates", source.source.id()));
    let path_data = |path: &geometry::Path| {
        let mut data = String::new();
        svg::push_rough_path(&mut data, path).expect("path data writes");
        data
    };
    drawing
        .fill
        .iter()
        .map(|fill| path_data(&fill.path))
        .chain(std::iter::once(path_data(&drawing.outline)))
        .collect()
}

/// Every SVG path of every element of `source`, in document order.
fn rough_svg_paths_of(source: &[u8]) -> Vec<String> {
    normalized_elements(source)
        .iter()
        .flat_map(rough_svg_paths)
        .collect()
}

/// The SVG path data a default render writes for one freedraw stroke.
fn freedraw_svg_path(source: &NormalizedElement) -> String {
    let NormalizedElementKind::Freedraw(freedraw) = &source.kind else {
        panic!("{} is a freedraw element", source.source.id());
    };
    let path = freedraw::generate(freedraw, source.stroke_width, &mut default_budget(source))
        .ok()
        .unwrap_or_else(|| panic!("{} generates", source.source.id()));
    svg::freedraw_path_data(&path).expect("freedraw path data writes")
}

fn sha256_hex(data: &str) -> String {
    crate::resource_accounting::sha256_hex(data.as_bytes())
}

/// The seven outline families the outline-shape render pins in every format:
/// a rectangle, a rounded rectangle, an ellipse, a diamond, a rounded diamond,
/// a line and a rounded line, none with a background.
const OUTLINE_FAMILIES: &[u8] = br##"{
  "type":"excalidraw","version":2,
  "elements":[
    {"id":"rectangle","type":"rectangle","x":10,"y":10,"width":80,"height":50,"seed":1,"updated":1},
    {"id":"rounded-rectangle","type":"rectangle","x":110,"y":10,"width":80,"height":50,"roundness":{"type":3},"seed":2,"updated":1},
    {"id":"ellipse","type":"ellipse","x":210,"y":10,"width":80,"height":50,"seed":3,"updated":1},
    {"id":"diamond","type":"diamond","x":310,"y":10,"width":80,"height":50,"seed":4,"updated":1},
    {"id":"rounded-diamond","type":"diamond","x":410,"y":10,"width":80,"height":50,"roundness":{"type":2},"seed":5,"updated":1},
    {"id":"line","type":"line","x":10,"y":90,"width":160,"height":40,"points":[[0,0],[80,40],[160,10]],"seed":6,"updated":1},
    {"id":"rounded-line","type":"line","x":210,"y":90,"width":160,"height":40,"points":[[0,0],[80,40],[160,10]],"roundness":{"type":2},"seed":7,"updated":1}
  ],
  "appState":{"viewBackgroundColor":"#ffffff"},"files":{}
}"##;

#[test]
fn outline_operations_match_the_pinned_roughjs_vectors() {
    let paths = rough_svg_paths_of(OUTLINE_FAMILIES);
    assert_eq!(paths.len(), 7);
    assert_eq!(
        paths
            .iter()
            .map(|path| path.matches(['M', 'C']).count())
            .collect::<Vec<_>>(),
        [16, 32, 24, 16, 32, 8, 6]
    );
    assert_eq!(
        sha256_hex(&paths.join("\n")),
        "6d0e81bef0d09500efd1e8f97e78d2a998331d4a4e7683d0adbf596a695fbe80",
        // The original vectors came from an independent pinned Rough.js 4.6.4 run,
        // but that run was fed midpoint diamond vertices, which is not what the
        // official export uses. The two diamond entries were re-derived after the
        // vertex fix; their geometry is corroborated by official evidence at two
        // sizes of differing parity, 170x110 and 130x95, where every path matches
        // byte for byte. The five non-diamond entries are unchanged from the
        // independent run.
        "diamond vectors re-derived against official evidence; the rest are the \
         independent pinned Rough.js 4.6.4 run"
    );
}

#[test]
fn architect_ellipse_uses_the_single_core_roughjs_stroke() {
    // Roughness 0 is Excalidraw's "architect" style: Rough.js draws the
    // ellipse's core stroke once instead of the sketchy double pass.
    let source = std::str::from_utf8(OUTLINE_FAMILIES).unwrap().replacen(
        "\"seed\":3,\"updated\"",
        "\"roughness\":0,\"seed\":3,\"updated\"",
        1,
    );
    let paths = rough_svg_paths_of(source.as_bytes());
    let ellipse = &paths[2];
    assert_eq!(ellipse.matches(['M', 'C']).count(), 42);
    assert_eq!(
        sha256_hex(ellipse),
        "d74c0380e58abcacfb14e495406d5f38af852f32f37a0ea528ecb0f0454bec05"
    );
}

#[test]
fn cartoonist_outline_consumes_vertex_offsets_in_roughjs_order() {
    // Roughness 2 is the "cartoonist" style: `preserveVertices` is off, so
    // Rough.js draws the corner offsets from its seeded generator in the
    // order the reference consumes them.
    let paths = rough_svg_paths_of(
        br##"{
        "type":"excalidraw","version":2,
        "elements":[{"id":"cartoonist","type":"rectangle","width":8,"height":5,
            "roughness":2,"seed":8,"updated":1}],
        "appState":{},"files":{}
    }"##,
    );
    assert_eq!(paths.len(), 1);
    assert_eq!(
        sha256_hex(&paths[0]),
        "fe33b396a5e457111305917fe234efdc6f856b635e7a55ab7fc5e5ed3083c19b"
    );
}

#[test]
fn cartoonist_rounded_outline_preserves_continuous_path_vertices() {
    // A rounded shape keeps `preserveVertices` on whatever its roughness, so
    // the path stays continuous through its arcs.
    let source = std::str::from_utf8(OUTLINE_FAMILIES).unwrap().replacen(
        "\"seed\":2,\"updated\"",
        "\"roughness\":2,\"seed\":2,\"updated\"",
        1,
    );
    let paths = rough_svg_paths_of(source.as_bytes());
    assert_eq!(
        sha256_hex(&paths[1]),
        "e9be3be60300e30a0f37ea1c58c8a7b18ddf966d468a0afb6629b3fc5f81aa7f"
    );
}

#[test]
fn rough_fill_vectors_match_the_pinned_geometry_families() {
    // One fill family per shape family: solid on a rounded rectangle,
    // hachure on an ellipse, cross-hatch on a diamond and zigzag on a closed
    // line loop. Each element writes its fill path and then its outline.
    let paths = rough_svg_paths_of(
        br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"rounded","type":"rectangle","x":10,"y":10,"width":80,"height":50,"roundness":{"type":3},"backgroundColor":"#e03131","fillStyle":"solid","seed":21,"updated":1},
        {"id":"ellipse","type":"ellipse","x":110,"y":10,"width":80,"height":50,"backgroundColor":"#e03131","fillStyle":"hachure","seed":22,"updated":1},
        {"id":"diamond","type":"diamond","x":210,"y":10,"width":80,"height":50,"backgroundColor":"#e03131","fillStyle":"cross-hatch","seed":23,"updated":1},
        {"id":"loop","type":"line","x":310,"y":10,"width":80,"height":50,"points":[[0,0],[80,0],[80,50],[0,50],[0,0]],"backgroundColor":"#e03131","fillStyle":"zigzag","seed":24,"updated":1}
      ],
      "appState":{},"files":{}
    }"##,
    );
    assert_eq!(paths.len(), 8);
    assert_eq!(
        sha256_hex(&paths.join("\n")),
        // Moved with the diamond vertex fix: the scene carries two diamonds.
        "d4e7528ff1a10f3ebbfbb8f3b404728ce7bef805930d56384652c2be6dc53f6d"
    );
}

#[test]
fn patterned_curved_boundaries_match_pinned_js_sampling() {
    // Hachure over a curved boundary samples the curve the way the pinned
    // Rough.js run does: the rounded rectangle's arcs and the rounded line's
    // spline are both fill boundaries here.
    let paths = rough_svg_paths_of(
        br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"rounded-path","type":"rectangle","x":10,"y":10,"width":80,"height":50,"roundness":{"type":3},"backgroundColor":"#e03131","fillStyle":"hachure","seed":31,"updated":1},
        {"id":"rounded-curve","type":"line","x":110,"y":10,"width":80,"height":50,"points":[[0,0],[80,0],[80,50],[0,50],[4,3]],"roundness":{"type":2},"backgroundColor":"#e03131","fillStyle":"hachure","seed":32,"updated":1}
      ],
      "appState":{},"files":{}
    }"##,
    );
    assert_eq!(paths.len(), 4);
    assert_eq!(
        sha256_hex(&paths[0]),
        "c211c4346fa692dc825164e947714aaed5b66e99ae2fcecdfe43ef656683bfb0",
        "rounded path fill was generated independently with pinned Rough.js 4.6.4",
    );
    assert_eq!(
        sha256_hex(&paths[2]),
        "9176fe003ade6804f0f3e9fe2b79aa67835300b4999ac2c72e43e639d0748f06",
        "rounded curve fill was generated independently with pinned Rough.js 4.6.4",
    );
}

#[test]
fn line_fill_uses_the_exact_export_zoom_loop_threshold() {
    // The export treats a line as closed, and fills it, when its last point
    // sits within `LINE_CONFIRM_THRESHOLD / zoom` of its first; `[6.4, 4.8]`
    // is exactly 8 away from the origin and `[6.408, 4.806]` just beyond.
    let elements = normalized_elements(
        br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"at-threshold","type":"line","x":10,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[6.4,4.8]],"backgroundColor":"#e03131","fillStyle":"solid","seed":51,"updated":1},
        {"id":"outside-threshold","type":"line","x":60,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[6.408,4.806]],"backgroundColor":"#e03131","fillStyle":"solid","seed":52,"updated":1}
      ],
      "appState":{},"files":{}
    }"##,
    );
    let drawing = |element: &NormalizedElement| {
        rough_drawing(
            element,
            adjusted_roughness(element),
            &mut default_budget(element),
        )
        .expect("line generates")
    };
    let closed = drawing(&elements[0]);
    assert!(closed.fill.is_some(), "the loop within the threshold fills");
    assert!(closed.shape.svg_uses_even_odd_fill());
    assert_eq!(rough_svg_paths(&elements[0]).len(), 2);
    let open = drawing(&elements[1]);
    assert!(open.fill.is_none(), "the loop beyond the threshold is open");
    assert_eq!(rough_svg_paths(&elements[1]).len(), 1);
}

#[test]
fn repeated_loop_endpoint_keeps_the_polygon_generators_extra_close() {
    // Rough.js closes a polygon by drawing back to its first point; a line
    // whose last point already repeats the first still gets that closing
    // segment, degenerate as it is, in both passes.
    let paths = rough_svg_paths_of(
        br##"{
      "type":"excalidraw","version":2,
      "elements":[
        {"id":"repeated","type":"line","x":10,"y":10,"width":40,"height":30,"points":[[0,0],[40,0],[40,30],[0,0]],"backgroundColor":"#e03131","fillStyle":"solid","seed":61,"updated":1}
      ],
      "appState":{},"files":{}
    }"##,
    );
    let [_fill, outline] = paths.as_slice() else {
        panic!("a filled polygon writes its fill and outline: {paths:?}");
    };
    assert!(
        outline.ends_with("M0 0 C0 0, 0 0, 0 0 M0 0 C0 0, 0 0, 0 0"),
        "unexpected repeated-close outline: {outline}",
    );
}

#[test]
fn freedraw_strokes_match_the_pinned_perfect_freehand_vectors() {
    // perfect-freehand 1.2.0 as Excalidraw v0.18.1 drives it: simulated
    // pressure, explicit pressure samples, a pressureless dot, and omitted
    // samples taking the library's defaults.
    for (source, expected_digest) in [
        (
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"simulated","type":"freedraw","x":10,"y":20,"width":80,"height":30,"points":[[0,0],[20,10],[40,5],[60,30],[80,20]],"pressures":[],"simulatePressure":true,"strokeColor":"#e03131","strokeWidth":2,"opacity":80,"updated":1}],"appState":{},"files":{}}"##.as_slice(),
            "9adff0c560710203718626bc817520baba2819a06898054e0b55d52a7cccd41e",
        ),
        (
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"pressure","type":"freedraw","width":50,"height":30,"points":[[0,0],[15,20],[35,10],[50,30]],"pressures":[0.1,0.4,0.9,0.3],"simulatePressure":false,"strokeWidth":3,"updated":1}],"appState":{},"files":{}}"##.as_slice(),
            "0bb4552d75b1f4b8a39f1d5721b052c288db07d3d51e7b5303ab3c6ffffd0f1e",
        ),
        (
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"dot","type":"freedraw","points":[],"pressures":[],"simulatePressure":false,"strokeWidth":3,"updated":1}],"appState":{},"files":{}}"##.as_slice(),
            "4a84f3104dec0d94b7a702c18b64304d0affeb8c00d21e8e08a4f0731236078a",
        ),
        (
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"defaults","type":"freedraw","width":40,"height":10,"points":[[0,0],[20,10],[40,5]],"simulatePressure":false,"strokeWidth":2,"updated":1}],"appState":{},"files":{}}"##.as_slice(),
            "98f047202f13fe696e5e6c715b343696f61f6460497f110bd2efae09e009289c",
        ),
    ] {
        let elements = normalized_elements(source);
        let path = freedraw_svg_path(&elements[0]);
        assert_eq!(
            sha256_hex(&path),
            expected_digest,
            "{}",
            elements[0].source.id()
        );
    }
}

#[test]
fn ellipse_ignores_inapplicable_roundness_without_consuming_randomness() {
    // The revision-1 manifest does not read `/roundness` on an ellipse, so a
    // rounded ellipse normalizes as a plain one and draws the same vectors:
    // the seeded generator is consumed identically.
    let rounded_source = std::str::from_utf8(OUTLINE_FAMILIES).unwrap().replacen(
        "\"seed\":3,\"updated\"",
        "\"roundness\":{\"type\":2},\"seed\":3,\"updated\"",
        1,
    );
    let plain = normalized_elements(OUTLINE_FAMILIES);
    let rounded = normalized_elements(rounded_source.as_bytes());
    assert_eq!(rounded[2].roundness, None);
    assert_eq!(rough_svg_paths(&plain[2]), rough_svg_paths(&rounded[2]));
}

#[test]
fn a_legacy_draw_element_normalizes_as_the_equivalent_line() {
    // Scenes saved before freedraw existed carry `type: "draw"`; the official
    // restore turns it into a line with the same points, so the two normalize
    // to the same linear kind and generate the same geometry.
    let source = |kind: &str| {
        format!(
            r#"{{"type":"excalidraw","version":2,"elements":[{{"id":"legacy","type":"{kind}","x":10,"y":20,"width":60,"height":20,"points":[[5,3],[30,20],[65,10]],"strokeWidth":2,"seed":17,"updated":1}}],"appState":{{}},"files":{{}}}}"#
        )
    };
    let legacy = normalized_elements(source("draw").as_bytes());
    let line = normalized_elements(source("line").as_bytes());
    assert_eq!(legacy[0].kind, line[0].kind);
    assert!(matches!(legacy[0].kind, NormalizedElementKind::Linear(_)));
    assert_eq!(
        (legacy[0].x, legacy[0].y, legacy[0].width, legacy[0].height),
        (line[0].x, line[0].y, line[0].width, line[0].height)
    );
    assert_eq!(rough_svg_paths(&legacy[0]), rough_svg_paths(&line[0]));
}
