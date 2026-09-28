//! Issue #82: bounded byte and grammar mutation through the public facade.
//! See docs/asset-fuzzing.md for the input envelope and failure promotion flow.

mod seeds;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::json;

use excalidraw_render::{
    DiagnosticCode, ErrorCode, OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest,
    Renderer, ResourceLimits, SceneDocument,
};

const FIXED_SEED: u64 = 0x3c6e_f372_fe94_f82b;
const MAX_PAYLOAD: usize = 262_144;
const MAX_SCENE: usize = 8 * 1_048_576;

#[derive(Clone)]
struct Seed {
    id: String,
    mime: &'static str,
    bytes: Vec<u8>,
    // Known-valid seeds must reach projection, rather than pass by rejection.
    valid: bool,
}

impl Seed {
    fn new(id: impl Into<String>, mime: &'static str, bytes: Vec<u8>, valid: bool) -> Self {
        let id = id.into();
        assert!(bytes.len() <= MAX_PAYLOAD, "{id}: {} bytes", bytes.len());
        Self {
            id,
            mime,
            bytes,
            valid,
        }
    }
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn scene(seed: &Seed, depth: usize, repeated: bool) -> Vec<u8> {
    let mut url = data_url(seed.mime, &seed.bytes);
    let mut mime = seed.mime;
    for _ in 0..depth {
        let image = format!(r#"<image width="8" height="8" href="{url}"/>"#);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">{image}{}</svg>"#,
            if repeated { &image } else { "" }
        );
        url = data_url("image/svg+xml", svg.as_bytes());
        mime = "image/svg+xml";
    }
    let mut elements = vec![json!({"id":"image","type":"image","x":0,"y":0,
        "width":8,"height":8,"fileId":"asset","status":"saved","scale":[1,1],
        "seed":1,"updated":1})];
    let mut files = json!({"asset":{"id":"asset","mimeType":mime,"dataURL":url,"created":1}});
    if repeated {
        let mut alias = elements[0].clone();
        alias["id"] = json!("alias");
        alias["fileId"] = json!("alias");
        alias["x"] = json!(8);
        elements.push(alias);
        files["alias"] = files["asset"].clone();
        files["alias"]["id"] = json!("alias");
    }
    let bytes = serde_json::to_vec(&json!({"type":"excalidraw","version":2,
        "elements":elements,"appState":{},"files":files}))
    .unwrap();
    assert!(
        bytes.len() <= MAX_SCENE,
        "{}: generator exceeded its envelope",
        seed.id
    );
    bytes
}

fn limits(mode: usize) -> ResourceLimits {
    let builder = ResourceLimits::builder()
        .max_scene_bytes(MAX_SCENE as u64)
        .max_asset_entries(64)
        .max_asset_bytes(262_144)
        .max_total_asset_bytes(1_048_576)
        .max_nested_asset_depth(4)
        .max_nested_svg_nodes(256)
        .max_raster_dimension(256)
        .max_decoded_pixels_per_asset(65_536)
        .max_total_decoded_pixels(131_072)
        .max_raster_surface_pixels(65_536)
        .max_total_rasterized_pixels(131_072)
        .max_diagnostics(128)
        .max_working_bytes(32 * 1_048_576)
        .max_output_bytes(2 * 1_048_576)
        .max_element_work_units(4_194_304)
        .max_asset_work_units(4_194_304)
        .max_total_work_units(16_777_216);
    match mode {
        0 => builder,
        1 => builder.max_asset_bytes(64),
        2 => builder.max_asset_bytes(128).max_total_asset_bytes(128),
        3 => builder.max_nested_asset_depth(1),
        4 => builder.max_nested_svg_nodes(2),
        5 => builder.max_decoded_pixels_per_asset(1),
        6 => builder
            .max_decoded_pixels_per_asset(2)
            .max_total_decoded_pixels(2),
        7 => builder.max_asset_work_units(64),
        8 => builder
            .max_element_work_units(128)
            .max_asset_work_units(128)
            .max_total_work_units(128),
        9 => builder.max_raster_surface_pixels(1),
        10 => builder
            .max_raster_surface_pixels(64)
            .max_total_rasterized_pixels(64),
        11 => builder.max_output_bytes(64),
        12 => builder.max_diagnostics(1),
        13 => builder.max_working_bytes(4096),
        14 => builder.max_asset_entries(1),
        _ => unreachable!(),
    }
    .build()
    .unwrap()
}

// Only failures allowed for these bounded image-only scenes. In particular,
// backend/internal failures and unsupported format *fatal* errors are bugs.
fn allowed_failure(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::RESOURCE_ASSET_COUNT_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_ASSET_BYTES_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_DECODED_PIXELS_EXCEEDED
            | ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_RASTERIZED_PIXELS_EXCEEDED
            | ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED
            | ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED
            | ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED
            | ErrorCode::PDF_RASTER_FALLBACK_FORBIDDEN
            | ErrorCode::PDF_RASTER_FALLBACK_UNISOLATABLE
    )
}

fn exercise(input: &[u8], mode: usize, forbid: bool, valid: bool) {
    let scene = SceneDocument::parse(input).expect("the generated scene envelope is valid");
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let limits = limits(mode);
        let mut builder = RenderRequest::builder(format)
            .padding(0.0)
            .include_background(false)
            .resource_limits(limits.clone());
        if format == OutputFormat::Pdf {
            builder = builder.pdf_options(PdfOptions::new(if forbid {
                PdfRasterFallback::Forbid
            } else {
                PdfRasterFallback::AllowIsolated
            }));
        }
        let request = builder.build().unwrap();
        let first = Renderer::new().render(&scene, &request);
        let second = Renderer::new().render(&scene, &request);
        let report = match (&first, &second) {
            (Ok(a), Ok(b)) => {
                assert_eq!(
                    a.artifact().bytes(),
                    b.artifact().bytes(),
                    "{format:?} artifact"
                );
                assert_eq!(a.report(), b.report(), "{format:?} report");
                let bytes = a.artifact().bytes();
                assert!(!bytes.is_empty() && bytes.len() as u64 <= limits.max_output_bytes());
                match format {
                    OutputFormat::Svg => inspect_svg(bytes, 0),
                    OutputFormat::Png => {
                        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
                        let mut reader = decoder.read_info().unwrap();
                        let mut pixels = vec![0; reader.output_buffer_size()];
                        let frame = reader.next_frame(&mut pixels).unwrap();
                        assert!(
                            u64::from(frame.width) * u64::from(frame.height)
                                <= limits.max_raster_surface_pixels()
                        );
                    }
                    OutputFormat::Pdf => {
                        assert!(bytes.starts_with(b"%PDF-1.7"));
                        assert!(bytes.ends_with(b"%%EOF"));
                    }
                    _ => unreachable!(),
                }
                if forbid {
                    assert!(!a
                        .report()
                        .diagnostics()
                        .iter()
                        .any(|d| d.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
                }
                if valid {
                    assert!(
                        a.report().diagnostics().iter().all(|d| matches!(
                            d.code(),
                            DiagnosticCode::IMAGE_ANIMATION_FLATTENED
                                | DiagnosticCode::PDF_RASTER_FALLBACK
                        )),
                        "valid seed recovered instead of reaching {format:?} projection: {:?}",
                        a.report()
                    );
                }
                a.report()
            }
            (Err(a), Err(b)) => {
                assert!(!valid, "valid seed failed: {a:?}");
                // Spec §12: failure to retain mandatory notices is an encode
                // failure even when a resource ceiling caused it.
                let encoding_code = match format {
                    OutputFormat::Svg => ErrorCode::ENCODE_SVG_FAILED,
                    OutputFormat::Png => ErrorCode::ENCODE_PNG_FAILED,
                    OutputFormat::Pdf => ErrorCode::ENCODE_PDF_FAILED,
                    _ => unreachable!(),
                };
                let notice_failure = matches!(mode, 8 | 11 | 13) && a.code() == encoding_code;
                if notice_failure {
                    // The same content must encode once notice-generation and
                    // retention budgets are restored. A backend failure is not
                    // made acceptable merely by choosing a low-limit profile.
                    let mut relaxed = RenderRequest::builder(format)
                        .padding(0.0)
                        .include_background(false)
                        .resource_limits(self::limits(0));
                    if format == OutputFormat::Pdf {
                        relaxed = relaxed.pdf_options(PdfOptions::new(if forbid {
                            PdfRasterFallback::Forbid
                        } else {
                            PdfRasterFallback::AllowIsolated
                        }));
                    }
                    let restored = Renderer::new().render(&scene, &relaxed.build().unwrap());
                    assert!(
                        restored.is_ok(),
                        "encoding failure survives restored notice budgets: {restored:?}"
                    );
                    assert!(
                        has_mandatory_notices(format, restored.unwrap().artifact().bytes()),
                        "encoding error without a mandatory-notice carrier"
                    );
                }
                assert!(
                    allowed_failure(a.code()) || notice_failure,
                    "unexpected {format:?} failure: {a:?}"
                );
                assert_eq!(
                    serde_json::to_value(a.descriptor()).unwrap(),
                    serde_json::to_value(b.descriptor()).unwrap()
                );
                assert_eq!(a.report(), b.report());
                a.report()
            }
            _ => panic!("{format:?} changed outcome kind"),
        };
        assert!(report.diagnostics().len() <= limits.max_diagnostics() as usize);
        for diagnostic in report.diagnostics() {
            let pointer = diagnostic.json_pointer();
            assert!(pointer.is_empty() || pointer.starts_with('/'));
            assert!(!diagnostic.effects().is_empty());
        }
    }
}

fn has_mandatory_notices(format: OutputFormat, bytes: &[u8]) -> bool {
    let manifest = match format {
        OutputFormat::Png => {
            let mut offset = 8;
            let mut found = None;
            while offset + 12 <= bytes.len() {
                let length =
                    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
                let data = &bytes[offset + 8..offset + 8 + length];
                let keyword = b"excalidraw-render:artifact-manifest\0";
                if &bytes[offset + 4..offset + 8] == b"iTXt" && data.starts_with(keyword) {
                    found = Some(
                        std::str::from_utf8(&data[keyword.len() + 4..])
                            .unwrap()
                            .to_owned(),
                    );
                }
                offset += 12 + length;
            }
            found
        }
        // The PDF manifest is the `artifact-manifest.json` attachment (ADR-0021).
        OutputFormat::Pdf => super::pdf_inspect::pdf_manifest(bytes),
        OutputFormat::Svg => {
            let text = String::from_utf8_lossy(bytes);
            let (start, end) = ("<excalidraw-render:manifest xmlns:excalidraw-render=\"urn:excalidraw-render:artifact-manifest:1\">", "</excalidraw-render:manifest>");
            text.split_once(start)
                .and_then(|(_, rest)| rest.split_once(end))
                .map(|(json, _)| {
                    let xml = format!("<root>{json}</root>");
                    roxmltree::Document::parse(&xml)
                        .unwrap()
                        .root_element()
                        .text()
                        .unwrap()
                        .to_owned()
                })
        }
        _ => unreachable!(),
    };
    manifest.is_some_and(|text| {
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        !value["notices"].as_array().unwrap().is_empty()
    })
}

/// Inspect what a downstream SVG consumer receives, recursively decoding only
/// embedded SVGs. Raster data URLs remain opaque source payloads.
fn inspect_svg(bytes: &[u8], depth: usize) {
    // Scene asset is depth zero in semantic accounting; the artifact root
    // and a generated leaf placeholder add two output-only levels.
    assert!(depth <= 6, "unbounded embedded SVG recursion");
    let text = std::str::from_utf8(bytes).unwrap();
    let doc = roxmltree::Document::parse(text).expect("artifact SVG is well formed");
    for node in doc.descendants().filter(|n| n.is_element()) {
        assert!(!matches!(
            node.tag_name().name(),
            "script" | "foreignObject" | "animate" | "set"
        ));
        for attr in node.attributes() {
            assert!(!attr.name().to_ascii_lowercase().starts_with("on"));
            if attr.name() == "href" {
                let value = attr.value();
                assert!(
                    value.starts_with('#') || value.starts_with("data:image/"),
                    "external href: {value}"
                );
                if let Some(encoded) = value.strip_prefix("data:image/svg+xml;base64,") {
                    inspect_svg(&STANDARD.decode(encoded).unwrap(), depth + 1);
                }
            }
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
                inspect_css(attr.value());
            }
        }
        if node.tag_name().name() == "style" {
            for child in node.children().filter(|n| n.is_text()) {
                inspect_css(child.text().unwrap());
            }
        }
    }
    assert!(
        !text.contains("fuzz.invalid"),
        "sanitizer retained a forbidden resource"
    );
}

fn inspect_css(source: &str) {
    crate::svg_resource_inspect::inspect_css(source, false);
}

fn replay(seed: &Seed, depth: usize, repeated: bool, mode: usize, forbid: bool, valid: bool) {
    let input = scene(seed, depth, repeated);
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("EXCALIDRAW_ASSET_FUZZ_TRACE").is_some() {
        use std::io::Write as _;
        eprintln!("asset fuzz {} depth={depth} repeated={repeated} mode={mode} forbid={forbid} valid={valid}\n{}", seed.id, String::from_utf8_lossy(&input));
        std::io::stderr().flush().unwrap();
    }
    // Emit a complete reproducer on failure, not a truncated preview. The
    // request is fully determined by mode + forbid, independent of RNG state.
    let result = std::panic::catch_unwind(|| exercise(&input, mode, forbid, valid));
    assert!(
        result.is_ok(),
        "asset fuzz {} depth={depth} repeated={repeated} mode={mode} forbid={forbid} valid={valid}\n{}",
        seed.id,
        String::from_utf8_lossy(&input)
    );
}

#[test]
fn admitted_seed_paths_reach_every_projection() {
    for seed in seeds::core() {
        replay(&seed, 0, false, 0, false, seed.valid);
        replay(&seed, 1, true, 0, false, seed.valid);
    }
}

struct Generator(u64);

impl Generator {
    fn next(&mut self) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        // Identical choice stream on 32- and 64-bit targets.
        (self.0 as u32) as usize
    }

    fn mutate(&mut self, seed: &Seed, strategy: usize) -> Seed {
        let mut seed = seed.clone();
        seed.valid = false;
        match strategy % 6 {
            0 => {} // Intact controls must remain in the generated population.
            1 => {
                let length = self.next() % (seed.bytes.len() + 1);
                seed.bytes.truncate(length);
            }
            2 => {
                for _ in 0..1 + self.next() % 8 {
                    if seed.bytes.is_empty() {
                        break;
                    }
                    let offset = self.next() % seed.bytes.len();
                    seed.bytes[offset] ^= 1 << (self.next() % 8);
                }
            }
            3 => {
                if !seed.bytes.is_empty() {
                    let offset = self.next() % seed.bytes.len();
                    let end = (offset + 1 + self.next() % 32).min(seed.bytes.len());
                    seed.bytes.splice(offset..end, [0xff; 4]);
                }
            }
            4 if seed.mime == "image/png" && seed.bytes.starts_with(b"\x89PNG\r\n\x1a\n") => {
                // Reseal CRCs so mutations reach decompression, frame and color
                // interpretation rather than stopping at the checksum gate.
                let mut offset = 8;
                let mut chunks = Vec::new();
                while offset + 12 <= seed.bytes.len() {
                    let length =
                        u32::from_be_bytes(seed.bytes[offset..offset + 4].try_into().unwrap())
                            as usize;
                    let Some(end) = offset.checked_add(12).and_then(|n| n.checked_add(length))
                    else {
                        break;
                    };
                    if end > seed.bytes.len() {
                        break;
                    }
                    if length > 0 {
                        chunks.push((offset, length));
                    }
                    offset = end;
                }
                if !chunks.is_empty() {
                    let (offset, length) = chunks[self.next() % chunks.len()];
                    let position = offset + 8 + self.next() % length;
                    seed.bytes[position] ^= 1 << (self.next() % 8);
                    let crc = crc32fast::hash(&seed.bytes[offset + 4..offset + 8 + length]);
                    seed.bytes[offset + 8 + length..offset + 12 + length]
                        .copy_from_slice(&crc.to_be_bytes());
                }
            }
            _ => {
                let offset = self.next() % (seed.bytes.len() + 1);
                let tokens: &[&[u8]] = &[
                    b"\0\0\0\0",
                    b"\xff\xff\xff\xff",
                    b"</svg>",
                    b"url(https://fuzz.invalid)",
                    b"&#x0;",
                    b"<!ENTITY",
                    b"RIFF",
                    b"ICC_PROFILE\0",
                ];
                seed.bytes.splice(
                    offset..offset,
                    tokens[self.next() % tokens.len()].iter().copied(),
                );
            }
        }
        seed.bytes.truncate(MAX_PAYLOAD);
        seed
    }
}

fn generated(cases: usize, expanded: bool) {
    let mut seeds = if expanded {
        seeds::expanded()
    } else {
        seeds::core()
    };
    if !expanded {
        seeds.extend(seeds::svg());
    }
    let mut generator = Generator(FIXED_SEED);
    let families = [
        "image/png",
        "image/jpeg",
        "image/gif",
        "image/webp",
        "image/bmp",
        "image/x-icon",
        "image/svg+xml",
    ];
    for index in 0..cases {
        // Round-robin families so the much larger PNG evidence population
        // cannot starve the other decoders or sanitizer of mutations.
        let family: Vec<_> = seeds
            .iter()
            .filter(|s| s.mime == families[index % families.len()])
            .collect();
        let original = family[generator.next() % family.len()];
        let mut seed = generator.mutate(original, index);
        seed.id = format!("seed={FIXED_SEED:016x} case={index} {}", original.id);
        // Bound exponential encoded size separately from admitted depth. Deep
        // chains are linear; repeated siblings use at most two levels.
        let repeated = generator.next().is_multiple_of(2);
        let depth = generator.next() % if repeated { 3 } else { 7 };
        let mode = generator.next() % 15;
        replay(
            &seed,
            depth,
            repeated,
            mode,
            generator.next().is_multiple_of(2),
            false,
        );
    }
}

#[test]
fn fixed_seed_asset_fuzz_smoke() {
    assert_asset_fuzz_smoke();
}

pub(crate) fn assert_asset_fuzz_smoke() {
    generated(64, false);
}

#[test]
fn sanitizer_and_limit_paths_are_bounded() {
    for seed in seeds::svg() {
        for mode in 0..15 {
            replay(&seed, 1, true, mode, mode.is_multiple_of(2), false);
        }
    }
}

#[test]
fn nested_depth_recovery_is_bounded() {
    let seed = &seeds::core()[0];
    replay(seed, 5, false, 0, false, false);
    assert_local(
        seed,
        5,
        0,
        DiagnosticCode::RESOURCE_NESTED_ASSET_DEPTH_EXCEEDED,
    );
}

fn assert_local(seed: &Seed, depth: usize, mode: usize, code: DiagnosticCode) {
    let scene = SceneDocument::parse(&scene(seed, depth, false)).unwrap();
    let request = RenderRequest::builder(OutputFormat::Png)
        .padding(0.0)
        .resource_limits(limits(mode))
        .build()
        .unwrap();
    let outcome = Renderer::new().render(&scene, &request).unwrap();
    let diagnostics = outcome.report().diagnostics();
    assert_eq!(diagnostics.len(), 1, "{}: {diagnostics:?}", seed.id);
    assert_eq!(diagnostics[0].code(), code, "{}", seed.id);
    assert_eq!(diagnostics[0].json_pointer(), "/files/asset/dataURL");
}

#[test]
fn authored_limit_crossings_have_exact_outcomes() {
    let seed = &seeds::core()[0];
    for (mode, code) in [
        (1, DiagnosticCode::RESOURCE_ASSET_BYTES_EXCEEDED),
        (5, DiagnosticCode::RESOURCE_IMAGE_PIXELS_EXCEEDED),
        (7, DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED),
    ] {
        assert_local(seed, 0, mode, code);
    }
    let scene = SceneDocument::parse(&scene(seed, 0, true)).unwrap();
    for (mode, code) in [
        (2, ErrorCode::RESOURCE_TOTAL_ASSET_BYTES_EXCEEDED),
        (8, ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED),
        (9, ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED),
        (14, ErrorCode::RESOURCE_ASSET_COUNT_EXCEEDED),
    ] {
        let request = RenderRequest::builder(OutputFormat::Png)
            .padding(0.0)
            .resource_limits(limits(mode))
            .build()
            .unwrap();
        let error = Renderer::new().render(&scene, &request).unwrap_err();
        assert_eq!(error.code(), code, "mode {mode}");
        assert!(error.details().is_some());
    }
}

#[test]
fn compressed_and_dimension_bombs_have_typed_outcomes() {
    use crate::codec_fixtures as f;
    let png = f::solid_png(1, 1, [255, 0, 0, 255]);
    let mut huge = png.clone();
    f::mutate_first_png_chunk(&mut huge, *b"IHDR", |data| {
        data[..8].copy_from_slice(&[0x7f, 0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff]);
    });
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&vec![0; 1_048_576], 9);
    let mut stream = png[..33].to_vec();
    f::append_png_chunk(&mut stream, *b"IDAT", &compressed);
    f::append_png_chunk(&mut stream, *b"IEND", &[]);
    let mut profile = b"bomb\0\0".to_vec();
    profile.extend_from_slice(&compressed);
    for (id, bytes) in [
        ("dimensions", huge),
        ("scanline-expansion", stream),
        (
            "icc-expansion",
            f::png_with_chunks_before_idat(&png, &[(*b"iCCP", profile)]),
        ),
    ] {
        let seed = Seed::new(id, "image/png", bytes, false);
        replay(&seed, 0, false, 0, false, false);
        replay(&seed, 1, true, 7, false, false);
        if id == "dimensions" {
            let scene = SceneDocument::parse(&scene(&seed, 0, false)).unwrap();
            let request = RenderRequest::builder(OutputFormat::Png)
                .resource_limits(limits(0))
                .build()
                .unwrap();
            let error = Renderer::new().render(&scene, &request).unwrap_err();
            // Intrinsic preflight memory precedes local dimension recovery.
            assert_eq!(error.code(), ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED);
            assert_eq!(error.json_pointer(), "/elements/0");
            assert!(error.report().diagnostics().is_empty());
        } else {
            assert_local(
                &seed,
                0,
                0,
                if id == "icc-expansion" {
                    DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT
                } else {
                    DiagnosticCode::IMAGE_DECODE_FAILED
                },
            );
        }
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "scheduled deterministic codec and nested SVG fuzz expansion"]
fn nightly_asset_fuzz_is_bounded_and_deterministic() {
    let cases = std::env::var("EXCALIDRAW_ASSET_FUZZ_CASES")
        .map(|v| v.parse::<usize>().expect("case count is an integer"))
        .unwrap_or(4096);
    assert!((1..=16_384).contains(&cases));
    // Guaranteed intact traversal of every seed, regardless of random choices.
    for seed in seeds::expanded() {
        replay(&seed, 0, false, 0, false, seed.valid);
        replay(&seed, 1, false, 0, false, seed.valid);
    }
    generated(cases, true);
}

#[test]
fn avif_is_rejected_in_revision_one() {
    let seed = Seed::new(
        "avif-rejection",
        "image/avif",
        b"\0\0\0\x18ftypavif\0\0\0\0avifmif1".to_vec(),
        false,
    );
    let scene = SceneDocument::parse(&scene(&seed, 0, false)).unwrap();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = Renderer::new()
            .render(&scene, &RenderRequest::builder(format).build().unwrap())
            .unwrap();
        assert!(outcome
            .report()
            .diagnostics()
            .iter()
            .any(|d| d.code() == DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT));
    }
}

#[test]
fn pdf_effect_seed_exercises_allowed_and_forbidden_fallback() {
    let seed = seeds::svg()
        .into_iter()
        .find(|s| s.id == "svg-filter")
        .unwrap();
    let scene = SceneDocument::parse(&scene(&seed, 0, false)).unwrap();
    let allowed = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(limits(0))
        .build()
        .unwrap();
    let outcome = Renderer::new().render(&scene, &allowed).unwrap();
    assert!(outcome
        .report()
        .diagnostics()
        .iter()
        .any(|d| d.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
    let forbidden = RenderRequest::builder(OutputFormat::Pdf)
        .resource_limits(limits(0))
        .pdf_options(PdfOptions::new(PdfRasterFallback::Forbid))
        .build()
        .unwrap();
    assert_eq!(
        Renderer::new()
            .render(&scene, &forbidden)
            .unwrap_err()
            .code(),
        ErrorCode::PDF_RASTER_FALLBACK_FORBIDDEN
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "replay a captured asset fuzz scene with its printed request"]
fn replay_captured_asset_fuzz_scene() {
    let path = std::env::var("EXCALIDRAW_ASSET_FUZZ_REPLAY").expect("scene path");
    let mode = std::env::var("EXCALIDRAW_ASSET_FUZZ_MODE")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    let forbid = std::env::var("EXCALIDRAW_ASSET_FUZZ_FORBID")
        .unwrap_or_else(|_| "false".into())
        .parse()
        .unwrap();
    let input = std::fs::read(path).unwrap();
    assert!(input.len() <= MAX_SCENE);
    let valid = std::env::var("EXCALIDRAW_ASSET_FUZZ_VALID")
        .unwrap_or_else(|_| "false".into())
        .parse()
        .unwrap();
    exercise(&input, mode, forbid, valid);
}
