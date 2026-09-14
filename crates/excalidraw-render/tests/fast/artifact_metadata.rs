use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorCode, ErrorDetails, MetadataOptions, OutputFormat, Recovery,
    RenderRequest, Renderer, ResourceLimits, SceneDocument, SupplementalAssets,
};
use sha2::{Digest, Sha256};

use crate::common::pdf_inspect::{inflate_streams, pdf_text};

const SOURCE: &[u8] = br#"{
  "type": "excalidraw",
  "version": 2,
  "source": "metadata-vector",
  "elements": [],
  "appState": {"future": 1.00e+02},
  "files": {"unreferenced": {"dataURL": "data:image/png;base64,AA=="}}
}"#;
const CANONICAL_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"source":"metadata-vector","elements":[],"appState":{"future":1.00e+02},"files":{"unreferenced":{"dataURL":"data:image/png;base64,AA=="}}}"#;
const MANDATORY_NOTICE_SOURCE: &[u8] = br#"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","width":100,"height":30,"fontSize":20,"fontFamily":3,"text":"Code","updated":1}],"appState":{},"files":{}}"#;

#[test]
fn every_format_round_trips_one_canonical_artifact_manifest() {
    let renderer = Renderer::new();
    let scene = SceneDocument::parse(SOURCE).expect("scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format)
            .metadata_options(MetadataOptions::new(true, true, true))
            .build()
            .expect("metadata request is valid");
        let outcome = renderer
            .render(&scene, &request)
            .expect("metadata render succeeds");
        let (manifest, embedded_scene) = extract(format, outcome.artifact().bytes());
        let scene_digest = format!("sha256:{:x}", Sha256::digest(CANONICAL_SCENE));
        let expected = format!(
            concat!(
                "{{\"schema_version\":1,",
                "\"provenance\":{{\"renderer_version\":\"{}\",",
                "\"compatibility_revision\":1,",
                "\"excalidraw_release\":\"v0.18.1\",",
                "\"reference_profile_digest\":\"{}\"}},",
                "\"diagnostic_summary\":{{\"snapshot\":\"pre_metadata\",",
                "\"total\":1,\"by_severity\":{{\"info\":0,\"warning\":1}},",
                "\"by_code\":[{{\"code\":\"compatibility.unsupported-property\",",
                "\"count\":1}}]}},\"notices\":[],",
                "\"embedded_scene\":{{\"name\":\"scene.excalidraw\",",
                "\"media_type\":\"application/vnd.excalidraw+json\",",
                "\"sha256\":\"{}\"}}}}"
            ),
            env!("CARGO_PKG_VERSION"),
            outcome.profile_digest(),
            scene_digest,
        );
        assert_eq!(manifest, expected, "{format:?} manifest");
        assert_eq!(embedded_scene, CANONICAL_SCENE, "{format:?} scene");
        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).unwrap();
                assert!(
                    svg.find("<!-- svg-source:excalidraw --><metadata>")
                        .unwrap()
                        < svg.find("<defs>").unwrap()
                );
            }
            OutputFormat::Png => assert_png_metadata_tail(outcome.artifact().bytes()),
            OutputFormat::Pdf => {
                let pdf = pdf_text(outcome.artifact().bytes());
                assert!(pdf.contains("/Names << /EmbeddedFiles << /Names [(scene.excalidraw)"));
                assert!(pdf.contains("/AFRelationship /Data"));
                assert!(pdf.contains("/EF << /F "));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn output_exhaustion_omits_only_the_crossing_optional_component() {
    let renderer = Renderer::new();
    let scene = SceneDocument::parse(SOURCE).expect("scene is admitted");
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let metadata = MetadataOptions::new(true, true, true);
        let unlimited_request = RenderRequest::builder(format)
            .metadata_options(metadata.clone())
            .build()
            .unwrap();
        let unlimited = renderer.render(&scene, &unlimited_request).unwrap();
        let output_limit = u64::try_from(unlimited.artifact().bytes().len()).unwrap() - 1;
        let limited_request = RenderRequest::builder(format)
            .metadata_options(metadata)
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(output_limit)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let limited = renderer.render(&scene, &limited_request).unwrap();
        let omission = limited
            .report()
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code() == DiagnosticCode::RESOURCE_OPTIONAL_METADATA_OMITTED
            })
            .expect("crossing metadata component is diagnosed");
        assert!(matches!(
            omission.details(),
            Some(ErrorDetails::ResourceLimit {
                field: "max_output_bytes",
                recovery: Some(Recovery::MetadataOmitted),
                ..
            })
        ));
        let manifest = manifest_without_scene(format, limited.artifact().bytes());
        assert!(manifest.contains("\"provenance\":{"));
        assert!(manifest.contains("\"diagnostic_summary\":{"));
        assert!(manifest.ends_with("\"embedded_scene\":null}"));
    }
}

#[test]
fn svg_exact_output_boundary_retains_the_complete_mandatory_notice_artifact() {
    let renderer = Renderer::new();
    let scene = SceneDocument::parse(MANDATORY_NOTICE_SOURCE).expect("font scene is admitted");
    let unlimited_request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let unlimited = renderer.render(&scene, &unlimited_request).unwrap();
    let output_bytes = u64::try_from(unlimited.artifact().bytes().len()).unwrap();
    let exact_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(
            ResourceLimits::builder()
                .max_output_bytes(output_bytes)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    let exact = renderer.render(&scene, &exact_request).unwrap();

    assert_eq!(exact.artifact().bytes(), unlimited.artifact().bytes());
    let manifest = artifact_manifest(OutputFormat::Svg, exact.artifact().bytes())
        .expect("mandatory notices are retained");
    assert!(manifest.contains("svg-font-subset@sha256:"));
}

#[test]
fn svg_one_byte_below_mandatory_notice_output_reports_the_resource_crossing() {
    let renderer = Renderer::new();
    let scene = SceneDocument::parse(MANDATORY_NOTICE_SOURCE).expect("font scene is admitted");
    let unlimited_request = RenderRequest::builder(OutputFormat::Svg).build().unwrap();
    let unlimited = renderer.render(&scene, &unlimited_request).unwrap();
    let output_bytes = u64::try_from(unlimited.artifact().bytes().len()).unwrap();
    let limit = output_bytes - 1;
    let limited_request = RenderRequest::builder(OutputFormat::Svg)
        .resource_limits(
            ResourceLimits::builder()
                .max_output_bytes(limit)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    let error = renderer.render(&scene, &limited_request).unwrap_err();

    assert_eq!(error.code(), ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED);
    assert!(matches!(
        error.details(),
        Some(ErrorDetails::ResourceLimit {
            field: "max_output_bytes",
            ..
        })
    ));
}

#[test]
fn svg_exact_measurement_preserves_optional_metadata_selection() {
    let renderer = Renderer::new();
    let scene = SceneDocument::parse(SOURCE).expect("scene is admitted");
    let metadata = MetadataOptions::new(true, true, true);
    let unlimited_request = RenderRequest::builder(OutputFormat::Svg)
        .metadata_options(metadata.clone())
        .build()
        .unwrap();
    let unlimited = renderer.render(&scene, &unlimited_request).unwrap();
    let limit = u64::try_from(unlimited.artifact().bytes().len()).unwrap() - 1;
    let limited_request = RenderRequest::builder(OutputFormat::Svg)
        .metadata_options(metadata)
        .resource_limits(
            ResourceLimits::builder()
                .max_output_bytes(limit)
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    let limited = renderer.render(&scene, &limited_request).unwrap();

    assert!(u64::try_from(limited.artifact().bytes().len()).unwrap() <= limit);
    let manifest = artifact_manifest(OutputFormat::Svg, limited.artifact().bytes()).unwrap();
    assert!(manifest.contains("\"provenance\":{"));
    assert!(manifest.contains("\"diagnostic_summary\":{"));
    assert!(manifest.ends_with("\"embedded_scene\":null}"));
    assert!(limited.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code() == DiagnosticCode::RESOURCE_OPTIONAL_METADATA_OMITTED
    }));
}

#[test]
fn rejected_higher_priority_metadata_does_not_block_a_later_component() {
    let renderer = Renderer::new();
    let scene = SceneDocument::parse(SOURCE).expect("scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let provenance = renderer
            .render(
                &scene,
                &RenderRequest::builder(format)
                    .metadata_options(MetadataOptions::new(false, true, false))
                    .build()
                    .unwrap(),
            )
            .unwrap();
        let summary = renderer
            .render(
                &scene,
                &RenderRequest::builder(format)
                    .metadata_options(MetadataOptions::new(false, false, true))
                    .build()
                    .unwrap(),
            )
            .unwrap();
        let limit = u64::try_from(summary.artifact().bytes().len()).unwrap();
        assert!(
            provenance.artifact().bytes().len() > summary.artifact().bytes().len(),
            "the vector must isolate the higher-priority component"
        );
        let request = RenderRequest::builder(format)
            .metadata_options(MetadataOptions::new(false, true, true))
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let outcome = renderer.render(&scene, &request).unwrap();
        let manifest = artifact_manifest(format, outcome.artifact().bytes())
            .expect("the later summary component is retained");
        assert!(
            manifest.contains("\"provenance\":null"),
            "{format:?}: provenance={}, summary={}, limited={}; manifest: {manifest}",
            provenance.artifact().bytes().len(),
            summary.artifact().bytes().len(),
            outcome.artifact().bytes().len(),
        );
        assert!(manifest.contains("\"diagnostic_summary\":{"));
        assert_eq!(
            outcome
                .report()
                .diagnostics()
                .iter()
                .filter(|diagnostic| {
                    diagnostic.code() == DiagnosticCode::RESOURCE_OPTIONAL_METADATA_OMITTED
                })
                .count(),
            1
        );
    }
}

#[test]
fn rasterized_font_glyphs_do_not_create_a_png_notice_manifest() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"text","type":"text","width":100,"height":30,"fontSize":20,"fontFamily":3,"text":"Code","updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("font scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let outcome = Renderer::new().render(&scene, &request).unwrap();
        let manifest = artifact_manifest(format, outcome.artifact().bytes());

        if format == OutputFormat::Png {
            assert_eq!(manifest, None, "PNG glyph pixels carry no font notice");
        } else {
            let manifest = manifest.expect("embedded font requires a manifest");
            let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
            let notices = manifest["notices"].as_array().unwrap();
            // Both vector formats carry the family's attribution notice under
            // its license identity and the OFL body once under
            // `license.ofl-1.1` (#151); the SVG adds the subset's Modified
            // Version notice, the PDF embeds the complete chunk and adds none.
            assert_eq!(notices[0]["asset_identity"], "font.cascadia-code.ofl-1.1");
            let attribution = notices[0]["text"].as_str().unwrap();
            assert!(attribution.starts_with(
                "Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font Name Cascadia Code. Licensed under the SIL Open Font License, Version 1.1; the license text is this manifest's license.ofl-1.1 notice."
            ));
            assert!(!attribution.contains("SIL OPEN FONT LICENSE Version 1.1"));
            assert_eq!(notices[1]["asset_identity"], "license.ofl-1.1");
            assert!(notices[1]["text"]
                .as_str()
                .unwrap()
                .starts_with("SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007"));
            if format == OutputFormat::Svg {
                assert_eq!(notices.len(), 3);
                assert!(notices[2]["asset_identity"]
                    .as_str()
                    .unwrap()
                    .starts_with("svg-font-subset@sha256:"));
                assert!(notices[2]["text"]
                    .as_str()
                    .unwrap()
                    .contains("names rewritten for compatibility revision 1"));
                assert!(notices[2]["text"].as_str().unwrap().contains(
                    "Source SHA-256: 5bce041e0fcce617e35efbc335b1372d27a84acb65fb464c65ff80ab3424a885"
                ));
            } else {
                assert_eq!(notices.len(), 2);
            }
        }
    }
}

#[test]
fn png_image_placeholders_carry_only_the_rendered_font_awesome_notices() {
    for (status, expected) in [
        (
            "pending",
            vec![(
                "font-awesome-free-5.15.4/solid/image@sha256:f3a4dff85d862e91501af10000d758898ca2bc3055ea451423ddedac7f41c1d9",
                "\"Image\" icon source",
            )],
        ),
        (
            "error",
            vec![
                (
                    "font-awesome-free-5.15.4/solid/ban@sha256:8b43245cb319c865fdc4ec865967bcd8b4e833c5b9e108adad750d3db3b64ca9",
                    "\"Ban\" icon source",
                ),
                (
                    "font-awesome-free-5.15.4/solid/image@sha256:f3a4dff85d862e91501af10000d758898ca2bc3055ea451423ddedac7f41c1d9",
                    "\"Image\" icon source",
                ),
            ],
        ),
    ] {
        let source = format!(
            "{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{{\"id\":\"image\",\"type\":\"image\",\"width\":20,\"height\":10,\"fileId\":\"missing\",\"status\":\"{status}\",\"updated\":1}}],\"appState\":{{}},\"files\":{{}}}}"
        );
        let scene = SceneDocument::parse(source.as_bytes()).expect("image scene is admitted");

        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let request = RenderRequest::builder(format).build().unwrap();
            let outcome = Renderer::new().render(&scene, &request).unwrap();
            let manifest = artifact_manifest(format, outcome.artifact().bytes());

            if format != OutputFormat::Png {
                assert_eq!(manifest, None, "unpainted placeholders carry no notice");
                continue;
            }
            let manifest: serde_json::Value =
                serde_json::from_str(&manifest.expect("painted placeholder requires notices"))
                    .unwrap();
            let notices = manifest["notices"].as_array().unwrap();
            assert_eq!(notices.len(), expected.len());
            for (notice, (identity, text_fragment)) in notices.iter().zip(&expected) {
                assert_eq!(notice["asset_identity"], *identity);
                let text = notice["text"].as_str().unwrap();
                assert!(text.contains("Font Awesome Free 5.15.4 by @fontawesome"));
                assert!(text.contains(text_fragment));
                assert!(text.contains("Licensed under CC BY 4.0"));
            }
        }
    }

    let zero_sized = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":0,"height":10,"fileId":"missing","status":"error","updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("zero-sized image scene is admitted");
    let request = RenderRequest::builder(OutputFormat::Png).build().unwrap();
    let outcome = Renderer::new().render(&zero_sized, &request).unwrap();
    assert_eq!(
        artifact_manifest(OutputFormat::Png, outcome.artifact().bytes()),
        None,
        "a placeholder without icon geometry carries no attribution"
    );

    let clipped = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":20,"height":10,"fileId":"missing","status":"error","frameId":"frame","updated":1},{"id":"frame","type":"frame","x":100,"width":10,"height":10,"updated":2}],"appState":{},"files":{}}"#,
    )
    .expect("clipped image scene is admitted");
    let outcome = Renderer::new().render(&clipped, &request).unwrap();
    assert_eq!(
        artifact_manifest(OutputFormat::Png, outcome.artifact().bytes()),
        None,
        "a fully clipped placeholder carries no attribution"
    );
}

#[test]
fn output_limit_cannot_silently_remove_a_mandatory_notice() {
    let font_scene = SceneDocument::parse(MANDATORY_NOTICE_SOURCE).unwrap();
    let image_scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":20,"height":10,"fileId":"missing","status":"error","updated":1}],"appState":{},"files":{}}"#,
    )
    .unwrap();

    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let scene = if format == OutputFormat::Png {
            &image_scene
        } else {
            &font_scene
        };
        let unlimited_request = RenderRequest::builder(format).build().unwrap();
        let unlimited = Renderer::new().render(scene, &unlimited_request).unwrap();
        let limit = u64::try_from(unlimited.artifact().bytes().len()).unwrap() - 1;
        let limited_request = RenderRequest::builder(format)
            .resource_limits(
                ResourceLimits::builder()
                    .max_output_bytes(limit)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let error = Renderer::new().render(scene, &limited_request).unwrap_err();
        assert_eq!(
            error.code(),
            match format {
                OutputFormat::Png => ErrorCode::ENCODE_PNG_FAILED,
                OutputFormat::Pdf => ErrorCode::ENCODE_PDF_FAILED,
                _ => unreachable!(),
            }
        );
        assert!(error.to_string().contains("mandatory notice"));
    }
}

fn artifact_manifest(format: OutputFormat, artifact: &[u8]) -> Option<String> {
    match format {
        OutputFormat::Svg => {
            let svg = std::str::from_utf8(artifact).unwrap();
            svg.split_once("<excalidraw-render:manifest xmlns:excalidraw-render=\"urn:excalidraw-render:artifact-manifest:1\">")
                .and_then(|(_, rest)| rest.split_once("</excalidraw-render:manifest>"))
                .map(|(manifest, _)| xml_unescape(manifest))
        }
        OutputFormat::Png => optional_png_manifest(artifact),
        OutputFormat::Pdf => {
            let pdf = pdf_text(artifact);
            pdf.split_once("<er:manifest>")
                .and_then(|(_, rest)| rest.split_once("</er:manifest>"))
                .map(|(manifest, _)| xml_unescape(manifest))
        }
        _ => unreachable!(),
    }
}

fn optional_png_manifest(png: &[u8]) -> Option<String> {
    let mut offset = 8;
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &png[offset + 4..offset + 8];
        let data = &png[offset + 8..offset + 8 + length];
        if kind == b"iTXt" && data.starts_with(b"excalidraw-render:artifact-manifest\0") {
            let keyword_end = data.iter().position(|byte| *byte == 0).unwrap();
            return Some(String::from_utf8(data[keyword_end + 5..].to_vec()).unwrap());
        }
        offset += 12 + length;
    }
    None
}

fn manifest_without_scene(format: OutputFormat, artifact: &[u8]) -> String {
    match format {
        OutputFormat::Svg => {
            let svg = std::str::from_utf8(artifact).unwrap();
            assert!(!svg.contains("<!-- payload-start -->"));
            xml_unescape(between(
                svg,
                "<excalidraw-render:manifest xmlns:excalidraw-render=\"urn:excalidraw-render:artifact-manifest:1\">",
                "</excalidraw-render:manifest>",
            ))
        }
        OutputFormat::Png => {
            assert!(!png_has_chunk_keyword(
                artifact,
                b"tEXt",
                b"application/vnd.excalidraw+json",
            ));
            extract_png_manifest(artifact)
        }
        OutputFormat::Pdf => {
            let pdf = pdf_text(artifact);
            assert!(!pdf.contains("/Type /EmbeddedFile"));
            xml_unescape(between(&pdf, "<er:manifest>", "</er:manifest>"))
        }
        _ => unreachable!(),
    }
}

#[test]
fn embedded_scene_excludes_supplements_and_keeps_source_spelling() {
    let scene = SceneDocument::parse(SOURCE).expect("scene is admitted");
    let supplements = SupplementalAssets::builder()
        .asset("detached", b"caller-only".to_vec())
        .build()
        .expect("supplement is valid");
    let request = RenderRequest::builder(OutputFormat::Png)
        .metadata_options(MetadataOptions::new(true, false, false))
        .supplemental_assets(supplements)
        .build()
        .expect("request is valid");

    let outcome = Renderer::new()
        .render(&scene, &request)
        .expect("render succeeds");
    let (_, embedded_scene) = extract(OutputFormat::Png, outcome.artifact().bytes());

    assert_eq!(embedded_scene, CANONICAL_SCENE);
    assert!(std::str::from_utf8(&embedded_scene)
        .expect("scene is UTF-8")
        .contains("1.00e+02"));
    assert!(!embedded_scene
        .windows(11)
        .any(|bytes| bytes == b"caller-only"));
}

pub(super) fn extract(format: OutputFormat, artifact: &[u8]) -> (String, Vec<u8>) {
    match format {
        OutputFormat::Svg => extract_svg(artifact),
        OutputFormat::Png => extract_png(artifact),
        OutputFormat::Pdf => extract_pdf(artifact),
        _ => panic!("unsupported test format"),
    }
}

fn extract_svg(svg: &[u8]) -> (String, Vec<u8>) {
    let svg = std::str::from_utf8(svg).expect("SVG is UTF-8");
    let payload = between(svg, "<!-- payload-start -->", "<!-- payload-end -->");
    let wrapper = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .expect("SVG payload is base64");
    let manifest = between(
        svg,
        "<excalidraw-render:manifest xmlns:excalidraw-render=\"urn:excalidraw-render:artifact-manifest:1\">",
        "</excalidraw-render:manifest>",
    );
    (xml_unescape(manifest), decode_scene_wrapper(&wrapper))
}

fn extract_png(png: &[u8]) -> (String, Vec<u8>) {
    let mut offset = 8;
    let mut wrapper = None;
    let mut manifest = None;
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &png[offset + 4..offset + 8];
        let data = &png[offset + 8..offset + 8 + length];
        if kind == b"tEXt" {
            let keyword_end = data.iter().position(|byte| *byte == 0).unwrap();
            if &data[..keyword_end] == b"application/vnd.excalidraw+json" {
                wrapper = Some(data[keyword_end + 1..].to_vec());
            }
        } else if kind == b"iTXt" {
            let keyword_end = data.iter().position(|byte| *byte == 0).unwrap();
            if &data[..keyword_end] == b"excalidraw-render:artifact-manifest" {
                manifest = Some(String::from_utf8(data[keyword_end + 5..].to_vec()).unwrap());
            }
        }
        offset += 12 + length;
    }
    (
        manifest.expect("PNG manifest exists"),
        decode_scene_wrapper(&wrapper.expect("PNG scene exists")),
    )
}

fn extract_png_manifest(png: &[u8]) -> String {
    optional_png_manifest(png).expect("PNG manifest exists")
}

fn png_has_chunk_keyword(png: &[u8], kind: &[u8; 4], keyword: &[u8]) -> bool {
    let mut offset = 8;
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        if &png[offset + 4..offset + 8] == kind
            && png[offset + 8..offset + 8 + length].starts_with(keyword)
        {
            return true;
        }
        offset += 12 + length;
    }
    false
}

fn assert_png_metadata_tail(png: &[u8]) {
    let mut offset = 8;
    let mut kinds = Vec::new();
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        kinds.push(&png[offset + 4..offset + 8]);
        offset += 12 + length;
    }
    let last_idat = kinds.iter().rposition(|kind| *kind == b"IDAT").unwrap();
    assert_eq!(&kinds[last_idat + 1..], &[b"tEXt", b"iTXt", b"IEND"]);
    assert_eq!(kinds.iter().filter(|kind| **kind == b"tEXt").count(), 1);
    assert_eq!(kinds.iter().filter(|kind| **kind == b"iTXt").count(), 1);
}

fn extract_pdf(pdf: &[u8]) -> (String, Vec<u8>) {
    let pdf = &inflate_streams(pdf);
    let text = String::from_utf8_lossy(pdf);
    let manifest = between(&text, "<er:manifest>", "</er:manifest>");
    let marker = b"/Type /EmbeddedFile\n/Subtype /application#2Fvnd.excalidraw+json\n";
    let start = pdf
        .windows(marker.len())
        .position(|bytes| bytes == marker)
        .unwrap();
    let stream = pdf[start..]
        .windows(7)
        .position(|bytes| bytes == b"stream\n")
        .map(|offset| start + offset + 7)
        .unwrap();
    let end = pdf[stream..]
        .windows(10)
        .position(|bytes| bytes == b"\nendstream")
        .map(|offset| stream + offset)
        .unwrap();
    (xml_unescape(manifest), pdf[stream..end].to_vec())
}

fn decode_scene_wrapper(wrapper: &[u8]) -> Vec<u8> {
    let prefix = b"{\"version\":\"1\",\"encoding\":\"bstring\",\"compressed\":true,\"encoded\":\"";
    let encoded = wrapper
        .strip_prefix(prefix)
        .and_then(|bytes| bytes.strip_suffix(b"\"}"))
        .expect("exact scene wrapper");
    let zlib = decode_json_byte_string(encoded);
    inflate_stored_zlib(&zlib)
}

fn decode_json_byte_string(encoded: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut offset = 0;
    while offset < encoded.len() {
        if encoded[offset] != b'\\' {
            output.push(encoded[offset]);
            offset += 1;
            continue;
        }
        offset += 1;
        match encoded[offset] {
            b'"' | b'\\' => output.push(encoded[offset]),
            b'b' => output.push(0x08),
            b't' => output.push(0x09),
            b'n' => output.push(0x0a),
            b'f' => output.push(0x0c),
            b'r' => output.push(0x0d),
            b'u' => {
                let digits = std::str::from_utf8(&encoded[offset + 1..offset + 5]).unwrap();
                output.push(u8::from_str_radix(digits, 16).unwrap());
                offset += 4;
            }
            _ => panic!("unexpected JSON escape"),
        }
        offset += 1;
    }
    output
}

fn inflate_stored_zlib(zlib: &[u8]) -> Vec<u8> {
    assert_eq!(&zlib[..2], &[0x78, 0x01]);
    let mut offset = 2;
    let mut output = Vec::new();
    loop {
        let final_block = zlib[offset] == 1;
        offset += 1;
        let length = u16::from_le_bytes(zlib[offset..offset + 2].try_into().unwrap()) as usize;
        let inverse = u16::from_le_bytes(zlib[offset + 2..offset + 4].try_into().unwrap());
        assert_eq!(inverse, !(length as u16));
        offset += 4;
        output.extend_from_slice(&zlib[offset..offset + length]);
        offset += length;
        if final_block {
            break;
        }
    }
    output
}

fn between<'a>(value: &'a str, prefix: &str, suffix: &str) -> &'a str {
    let start = value.find(prefix).expect("prefix exists") + prefix.len();
    let tail = &value[start..];
    &tail[..tail.find(suffix).expect("suffix exists")]
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
