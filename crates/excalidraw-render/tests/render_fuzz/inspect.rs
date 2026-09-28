use base64::{engine::general_purpose::STANDARD, Engine as _};
use excalidraw_render::{
    CompatibilityReport, DiagnosticCode, DiagnosticSeverity, OutputFormat, RenderRequest,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    text.split_once(start)?
        .1
        .split_once(end)
        .map(|(body, _)| body)
}

fn png_chunks(bytes: &[u8]) -> Vec<(&[u8], &[u8])> {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let mut chunks = Vec::new();
    let mut cursor = 8;
    while cursor < bytes.len() {
        let length = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let end = cursor + 8 + length;
        let crc = u32::from_be_bytes(bytes[end..end + 4].try_into().unwrap());
        assert_eq!(crc32fast::hash(&bytes[cursor + 4..end]), crc);
        chunks.push((&bytes[cursor + 4..cursor + 8], &bytes[cursor + 8..end]));
        cursor = end + 4;
    }
    assert_eq!(chunks.last().unwrap().0, b"IEND");
    assert!(chunks.last().unwrap().1.is_empty());
    chunks
}

pub(super) fn manifest(format: OutputFormat, bytes: &[u8]) -> Option<Value> {
    match format {
        OutputFormat::Svg => {
            let doc = roxmltree::Document::parse(std::str::from_utf8(bytes).unwrap()).unwrap();
            doc.descendants()
                .find(|n| n.tag_name().name() == "manifest")
                .map(|n| serde_json::from_str(n.text().unwrap()).unwrap())
        }
        OutputFormat::Png => png_chunks(bytes).into_iter().find_map(|(kind, data)| {
            if kind != b"iTXt" {
                return None;
            }
            let payload = data.strip_prefix(b"excalidraw-render:artifact-manifest\0\0\0\0\0")?;
            Some(serde_json::from_slice(payload).unwrap())
        }),
        // Attachments, not XMP, carry PDF metadata (ADR-0021).
        OutputFormat::Pdf => {
            crate::pdf_inspect::pdf_manifest(bytes).map(|text| serde_json::from_str(&text).unwrap())
        }
        _ => unreachable!(),
    }
}

fn scene_wrapper(bytes: &[u8]) -> Vec<u8> {
    // The carrier's bstring is byte-valued JSON, including raw bytes >= 128;
    // it is deliberately not an ordinary UTF-8 JSON string.
    let prefix = b"{\"version\":\"1\",\"encoding\":\"bstring\",\"compressed\":true,\"encoded\":\"";
    let encoded = bytes
        .strip_prefix(prefix)
        .unwrap()
        .strip_suffix(b"\"}")
        .unwrap();
    let mut zlib = Vec::new();
    let mut cursor = 0;
    while cursor < encoded.len() {
        let byte = encoded[cursor];
        cursor += 1;
        if byte != b'\\' {
            zlib.push(byte);
            continue;
        }
        let escape = encoded[cursor];
        cursor += 1;
        zlib.push(match escape {
            b'"' | b'\\' => escape,
            b'b' => 8,
            b't' => 9,
            b'n' => 10,
            b'f' => 12,
            b'r' => 13,
            b'u' => {
                let value = u8::from_str_radix(
                    std::str::from_utf8(&encoded[cursor..cursor + 4]).unwrap(),
                    16,
                )
                .unwrap();
                cursor += 4;
                value
            }
            _ => panic!("invalid bstring escape"),
        });
    }
    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&zlib, super::MAX_INPUT).unwrap()
}

fn embedded_scene(format: OutputFormat, bytes: &[u8]) -> Option<Vec<u8>> {
    match format {
        OutputFormat::Svg => {
            let payload = between(
                std::str::from_utf8(bytes).unwrap(),
                "<!-- payload-start -->",
                "<!-- payload-end -->",
            )?;
            Some(scene_wrapper(&STANDARD.decode(payload).unwrap()))
        }
        OutputFormat::Png => png_chunks(bytes).into_iter().find_map(|(kind, data)| {
            if kind != b"tEXt" {
                return None;
            }
            data.strip_prefix(b"application/vnd.excalidraw+json\0")
                .map(scene_wrapper)
        }),
        OutputFormat::Pdf => crate::pdf_inspect::pdf_attachment(bytes, "scene.excalidraw"),
        _ => unreachable!(),
    }
}

fn svg(bytes: &[u8], depth: usize) {
    assert!(depth <= 4);
    let text = std::str::from_utf8(bytes).unwrap();
    let doc = roxmltree::Document::parse(text).unwrap();
    assert_eq!(doc.root_element().tag_name().name(), "svg");
    for node in doc.descendants().filter(|n| n.is_element()) {
        assert!(!matches!(
            node.tag_name().name(),
            "script" | "foreignObject" | "animate" | "set"
        ));
        for attr in node.attributes() {
            assert!(!attr.name().to_ascii_lowercase().starts_with("on"));
            if matches!(
                attr.name(),
                "style"
                    | "fill"
                    | "stroke"
                    | "filter"
                    | "mask"
                    | "clip-path"
                    | "cursor"
                    | "marker-start"
                    | "marker-mid"
                    | "marker-end"
            ) {
                crate::svg_resource_inspect::inspect_css(attr.value(), false);
            }
            if attr.name() == "href" {
                let href = attr.value();
                if node.tag_name().name() == "a" {
                    assert!(!href.to_ascii_lowercase().contains("javascript:"));
                } else {
                    assert!(
                        href.starts_with('#') || href.starts_with("data:image/"),
                        "external resource {href}"
                    );
                }
                if let Some(payload) = href.strip_prefix("data:image/svg+xml;base64,") {
                    svg(&STANDARD.decode(payload).unwrap(), depth + 1);
                }
            }
        }
        if node.tag_name().name() == "style" {
            for child in node.children().filter(|n| n.is_text()) {
                crate::svg_resource_inspect::inspect_css(child.text().unwrap(), depth == 0);
            }
        }
    }
}

pub(super) fn artifact(
    format: OutputFormat,
    bytes: &[u8],
    input: &[u8],
    request: &RenderRequest,
    report: &CompatibilityReport,
) {
    match format {
        OutputFormat::Svg => svg(bytes, 0),
        OutputFormat::Png => {
            png_chunks(bytes);
            let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            let info = decoder.info();
            assert!(
                u64::from(info.width) * u64::from(info.height)
                    <= request.resource_limits().max_raster_surface_pixels()
            );
            let mut pixels = vec![0; decoder.output_buffer_size()];
            decoder.next_frame(&mut pixels).unwrap();
        }
        OutputFormat::Pdf => {
            assert!(bytes.starts_with(b"%PDF-1.7") && bytes.ends_with(b"%%EOF"));
            let text = crate::pdf_inspect::pdf_text(bytes);
            assert!(text.contains("/Type /Catalog") && text.contains("/Type /Page\n"));
            assert!(text.contains("/Count 1\n"));
            // Validate that the terminal xref pointer addresses the actual xref.
            let raw = String::from_utf8_lossy(bytes);
            let offset: usize = between(&raw, "startxref\n", "\n%%EOF")
                .unwrap()
                .parse()
                .unwrap();
            assert!(bytes[offset..].starts_with(b"xref\n"));
        }
        _ => unreachable!(),
    }
    let metadata = request.metadata_options();
    let omissions = report
        .diagnostics()
        .iter()
        .filter(|d| d.code() == DiagnosticCode::RESOURCE_OPTIONAL_METADATA_OMITTED)
        .count();
    let embedded = embedded_scene(format, bytes);
    let Some(manifest) = manifest(format, bytes) else {
        assert!(embedded.is_none(), "orphaned embedded scene carrier");
        assert_eq!(
            omissions,
            usize::from(metadata.embedded_scene())
                + usize::from(metadata.provenance())
                + usize::from(metadata.diagnostic_summary())
        );
        return;
    };
    assert_eq!(manifest["schema_version"], 1);
    let mut missing = 0;
    for (key, requested) in [
        ("provenance", metadata.provenance()),
        ("diagnostic_summary", metadata.diagnostic_summary()),
        ("embedded_scene", metadata.embedded_scene()),
    ] {
        if !requested {
            assert!(manifest[key].is_null());
        } else if manifest[key].is_null() {
            missing += 1;
        }
    }
    assert_eq!(
        missing, omissions,
        "every omitted component needs its own diagnostic"
    );
    if !manifest["provenance"].is_null() {
        assert_eq!(manifest["provenance"]["compatibility_revision"], 1);
    }
    if !manifest["diagnostic_summary"].is_null() {
        let diagnostics: Vec<_> = report
            .diagnostics()
            .iter()
            .filter(|d| {
                !d.code().as_str().starts_with("metadata.")
                    && d.code() != DiagnosticCode::RESOURCE_OPTIONAL_METADATA_OMITTED
            })
            .collect();
        let mut counts = BTreeMap::new();
        let mut info = 0;
        for diagnostic in &diagnostics {
            *counts.entry(diagnostic.code().as_str()).or_insert(0usize) += 1;
            if diagnostic.severity() == DiagnosticSeverity::Info {
                info += 1;
            }
        }
        let by_code: Vec<_> = counts
            .into_iter()
            .map(|(code, count)| json!({"code":code,"count":count}))
            .collect();
        assert_eq!(
            manifest["diagnostic_summary"],
            json!({"snapshot":"pre_metadata", "total":diagnostics.len(),
            "by_severity":{"info":info,"warning":diagnostics.len()-info}, "by_code":by_code})
        );
    }
    assert_eq!(embedded.is_some(), !manifest["embedded_scene"].is_null());
    if let Some(original) = embedded {
        assert_eq!(
            serde_json::from_slice::<Value>(&original).unwrap(),
            serde_json::from_slice::<Value>(input).unwrap()
        );
        assert_eq!(
            manifest["embedded_scene"]["sha256"],
            format!("sha256:{:x}", Sha256::digest(&original))
        );
    }
}
