use base64::Engine as _;
use excalidraw_render::{
    DiagnosticCode, ErrorDetails, MetadataOptions, OutputFormat, Recovery, RenderRequest, Renderer,
    ResourceLimits, SceneDocument, SupplementalAssets,
};
use sha2::{Digest, Sha256};

const SOURCE: &[u8] = br#"{
  "type": "excalidraw",
  "version": 2,
  "source": "metadata-vector",
  "elements": [],
  "appState": {"future": 1.00e+02},
  "files": {"unreferenced": {"dataURL": "data:image/png;base64,AA=="}}
}"#;
const CANONICAL_SCENE: &[u8] = br#"{"type":"excalidraw","version":2,"source":"metadata-vector","elements":[],"appState":{"future":1.00e+02},"files":{"unreferenced":{"dataURL":"data:image/png;base64,AA=="}}}"#;

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
                let pdf = String::from_utf8_lossy(outcome.artifact().bytes());
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
            let pdf = String::from_utf8_lossy(artifact);
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

fn extract(format: OutputFormat, artifact: &[u8]) -> (String, Vec<u8>) {
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
    let mut offset = 8;
    while offset < png.len() {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &png[offset + 4..offset + 8];
        let data = &png[offset + 8..offset + 8 + length];
        if kind == b"iTXt" && data.starts_with(b"excalidraw-render:artifact-manifest\0") {
            let keyword_end = data.iter().position(|byte| *byte == 0).unwrap();
            return String::from_utf8(data[keyword_end + 5..].to_vec()).unwrap();
        }
        offset += 12 + length;
    }
    panic!("PNG manifest exists")
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
