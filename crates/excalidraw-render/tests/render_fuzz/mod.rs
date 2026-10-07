//! Bounded complete requests, replayed in reverse format and case order.
mod generator;
mod inspect;
mod regressions;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use excalidraw_render::{
    ColorMode, CompatibilityReport, DiagnosticCode, ErrorCode, FrameOptions, MetadataOptions,
    OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest, Renderer, ResourceLimits,
    SceneDocument, Sizing, SupplementalAssets,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

const MAX_INPUT: usize = 65_536;
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const LIMIT_MODES: usize = 13;

/// The complete reproducer, including request choices; no generator is needed
/// to replay it. Base64 also preserves malformed UTF-8 and exact number tokens.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    scene_base64: String,
    options: u32,
    limit_mode: usize,
    #[serde(default)]
    output_ceilings: Option<[u64; 3]>,
}

fn limits(mode: usize, output: Option<u64>) -> ResourceLimits {
    let builder = ResourceLimits::builder()
        .max_scene_bytes(MAX_INPUT as u64)
        .max_elements(32)
        .max_asset_entries(16)
        .max_asset_bytes(65_536)
        .max_total_asset_bytes(131_072)
        .max_relationship_depth(8)
        .max_nested_asset_depth(2)
        .max_nested_svg_nodes(128)
        .max_geometry_segments_per_element(4096)
        .max_total_geometry_segments(16_384)
        .max_text_scalars_per_element(128)
        .max_total_text_scalars(512)
        .max_raster_dimension(256)
        .max_decoded_pixels_per_asset(65_536)
        .max_total_decoded_pixels(131_072)
        .max_raster_surface_pixels(65_536)
        .max_total_rasterized_pixels(262_144)
        .max_diagnostics(128)
        .max_working_bytes(256 * 1_048_576)
        .max_output_bytes(output.unwrap_or(8 * 1_048_576))
        .max_element_work_units(16_777_216)
        .max_asset_work_units(16_777_216)
        .max_total_work_units(1_073_741_824);
    match mode {
        0 | 12 => builder,
        1 => builder.max_scene_bytes(64),
        2 => builder.max_elements(1),
        3 => builder
            .max_geometry_segments_per_element(1)
            .max_total_geometry_segments(1),
        4 => builder
            .max_text_scalars_per_element(1)
            .max_total_text_scalars(1),
        5 => builder.max_diagnostics(1),
        6 => builder.max_working_bytes(4096),
        7 => builder.max_output_bytes(64),
        8 => builder
            .max_element_work_units(64)
            .max_asset_work_units(64)
            .max_total_work_units(64),
        9 => builder.max_raster_surface_pixels(1),
        10 => builder.max_asset_bytes(64),
        11 => builder.max_element_work_units(128),
        _ => panic!("unknown limit mode"),
    }
    .build()
    .unwrap()
}

fn request(case: &Case, format: OutputFormat, output: Option<u64>) -> RenderRequest {
    let bits = case.options;
    let mut builder = RenderRequest::builder(format)
        .padding(f64::from(1 + (bits >> 3) % 4))
        .include_background(bits & 64 != 0)
        .color_mode(if bits & 128 == 0 {
            ColorMode::Light
        } else {
            ColorMode::Dark
        })
        .sizing(if bits & 256 == 0 {
            Sizing::Scale(0.5)
        } else {
            Sizing::MaxDimension(96.0)
        })
        .frame_options(
            FrameOptions::default()
                .with_enabled(bits & 512 == 0)
                .with_clip(bits & 1024 == 0)
                .with_show_name(bits & 2048 == 0)
                .with_outline(bits & 4096 == 0),
        )
        .metadata_options(
            MetadataOptions::default()
                .with_embedded_scene(bits & 1 != 0)
                .with_provenance(bits & 2 != 0)
                .with_diagnostic_summary(bits & 4 != 0),
        )
        .resource_limits(limits(case.limit_mode, output));
    if bits & 8192 != 0 {
        builder = builder.supplemental_assets(
            SupplementalAssets::builder()
                .asset_with_media_type(
                    "asset",
                    b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>"
                        .to_vec(),
                    "image/svg+xml",
                )
                .build()
                .unwrap(),
        );
    }
    if format == OutputFormat::Pdf {
        builder = builder.pdf_options(PdfOptions::default().with_raster_fallback(
            if bits & 16384 == 0 {
                PdfRasterFallback::AllowIsolated
            } else {
                PdfRasterFallback::Forbid
            },
        ));
    }
    builder.build().unwrap()
}

#[derive(Debug, PartialEq)]
enum Snapshot {
    Rejected(Vec<u8>),
    Artifact([u8; 32], Vec<u8>),
    Failure(Vec<u8>, Vec<u8>),
}

fn report_bytes(
    report: &CompatibilityReport,
    format: OutputFormat,
    limits: &ResourceLimits,
) -> Vec<u8> {
    assert!(report.len() <= limits.max_diagnostics() as usize);
    for diagnostic in report.diagnostics() {
        assert_eq!(diagnostic.format(), format);
        assert!(!diagnostic.effects().is_empty());
        assert!(diagnostic.json_pointer().is_empty() || diagnostic.json_pointer().starts_with('/'));
    }
    serde_json::to_vec(report).unwrap()
}

fn allowed_failure(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::RESOURCE_SCENE_BYTES_EXCEEDED
            | ErrorCode::RESOURCE_ELEMENT_COUNT_EXCEEDED
            | ErrorCode::RESOURCE_ASSET_COUNT_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_ASSET_BYTES_EXCEEDED
            | ErrorCode::RESOURCE_RELATIONSHIP_DEPTH_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_TEXT_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_DECODED_PIXELS_EXCEEDED
            | ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_RASTERIZED_PIXELS_EXCEEDED
            | ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED
            | ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED
            | ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED
            | ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED
            | ErrorCode::RESOURCE_COORDINATE_RANGE_EXCEEDED
            | ErrorCode::RESOURCE_SVG_DIMENSIONS_EXCEEDED
            | ErrorCode::RESOURCE_PDF_PAGE_DIMENSIONS_EXCEEDED
            | ErrorCode::PDF_RASTER_FALLBACK_FORBIDDEN
            | ErrorCode::PDF_RASTER_FALLBACK_UNISOLATABLE
    )
}

fn exercise(case: &Case, renderer: &Renderer, reverse: bool) -> Vec<Snapshot> {
    let input = STANDARD.decode(&case.scene_base64).unwrap();
    assert!(input.len() <= MAX_INPUT);
    let scene = match SceneDocument::parse(&input) {
        Ok(scene) => scene,
        Err(error) => {
            assert!(error.code().as_str().starts_with("scene."));
            return vec![Snapshot::Rejected(
                serde_json::to_vec(&error.descriptor()).unwrap(),
            )];
        }
    };
    let mut snapshots = Vec::new();
    for format in if reverse {
        [FORMATS[2], FORMATS[1], FORMATS[0]]
    } else {
        FORMATS
    } {
        // One byte below the complete artifact probes optional-metadata
        // retention and late atomic failure, with identical scene and options.
        let output = case
            .output_ceilings
            .map(|ceilings| ceilings[FORMATS.iter().position(|f| *f == format).unwrap()]);
        let request = request(case, format, output);
        let snapshot = match renderer.render(&scene, &request) {
            Ok(outcome) => {
                assert_eq!(outcome.artifact().format(), format);
                let bytes = outcome.artifact().bytes();
                assert!(
                    !bytes.is_empty()
                        && bytes.len() as u64 <= request.resource_limits().max_output_bytes()
                );
                inspect::artifact(format, bytes, &input, &request, outcome.report());
                if case.options & 16384 != 0 {
                    assert!(!outcome
                        .report()
                        .diagnostics()
                        .iter()
                        .any(|d| d.code() == DiagnosticCode::PDF_RASTER_FALLBACK));
                }
                Snapshot::Artifact(
                    Sha256::digest(bytes).into(),
                    report_bytes(outcome.report(), format, request.resource_limits()),
                )
            }
            Err(error) => {
                let encoding = match format {
                    OutputFormat::Svg => ErrorCode::ENCODE_SVG_FAILED,
                    OutputFormat::Png => ErrorCode::ENCODE_PNG_FAILED,
                    OutputFormat::Pdf => ErrorCode::ENCODE_PDF_FAILED,
                    _ => unreachable!(),
                };
                if error.code() == encoding && matches!(case.limit_mode, 6..=8 | 12) {
                    let mut relaxed = case.clone();
                    relaxed.limit_mode = 0;
                    let restored = renderer
                        .render(&scene, &self::request(&relaxed, format, None))
                        .expect("encoding failure must disappear with restored notice budgets");
                    assert!(inspect::manifest(format, restored.artifact().bytes())
                        .is_some_and(|m| !m["notices"].as_array().unwrap().is_empty()));
                } else {
                    assert!(
                        allowed_failure(error.code()),
                        "unexpected failure: {error:?}"
                    );
                }
                assert_eq!(error.format(), format);
                // Only the typed descriptor and report are exposed by a failure;
                // this public variant has no artifact or writer to publish to.
                Snapshot::Failure(
                    serde_json::to_vec(&error.descriptor()).unwrap(),
                    report_bytes(error.report(), format, request.resource_limits()),
                )
            }
        };
        snapshots.push(snapshot);
    }
    if reverse {
        snapshots.reverse();
    }
    snapshots
}

fn replay(case: &Case, renderer: &Renderer, reverse: bool) -> Vec<Snapshot> {
    let case = prepare(case.clone());
    let captured = serde_json::to_string(&case).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("EXCALIDRAW_RENDER_FUZZ_TRACE").is_some() {
        use std::io::Write as _;
        eprintln!("render-fuzz {captured}");
        std::io::stderr().flush().unwrap();
    }
    std::panic::catch_unwind(|| exercise(&case, renderer, reverse))
        .unwrap_or_else(|_| panic!("render-fuzz reproducer: {captured}"))
}

fn prepare(mut case: Case) -> Case {
    if case.limit_mode == 12 && case.output_ceilings.is_none() {
        let input = STANDARD.decode(&case.scene_base64).unwrap();
        assert!(input.len() <= MAX_INPUT);
        if let Ok(scene) = SceneDocument::parse(&input) {
            let mut relaxed = case.clone();
            relaxed.limit_mode = 0;
            let result = std::panic::catch_unwind(|| {
                FORMATS.map(|format| {
                    Renderer::new()
                        .render(&scene, &request(&relaxed, format, None))
                        .ok()
                        .map_or(8 * 1_048_576, |o| {
                            (o.artifact().bytes().len() as u64).saturating_sub(1).max(1)
                        })
                })
            });
            case.output_ceilings = Some(result.unwrap_or_else(|_| {
                panic!(
                    "boundary preparation; render-fuzz reproducer: {}",
                    serde_json::to_string(&relaxed).unwrap()
                )
            }));
        }
    }
    case
}

fn sequence(cases: &[Case]) {
    let renderer = Renderer::new();
    let mut first = Vec::new();
    for (position, case) in cases.iter().enumerate() {
        first.push(
            std::panic::catch_unwind(|| replay(case, &renderer, false)).unwrap_or_else(|_| {
                panic!(
                    "forward position={position}; render-fuzz sequence: {}",
                    serde_json::to_string(cases).unwrap()
                )
            }),
        );
    }
    for (position, (case, expected)) in cases.iter().zip(&first).enumerate().rev() {
        let actual = std::panic::catch_unwind(|| replay(case, &Renderer::new(), true))
            .unwrap_or_else(|_| {
                panic!(
                    "reverse position={position}; render-fuzz sequence: {}",
                    serde_json::to_string(cases).unwrap()
                )
            });
        assert_eq!(
            &actual,
            expected,
            "position={position}; render-fuzz sequence: {}",
            serde_json::to_string(cases).unwrap()
        );
    }
}

fn generated(count: usize) {
    let mut generator = generator::Generator::new();
    let cases: Vec<_> = (0..count)
        .map(|index| {
            let mut case = generator.case(index);
            // First cycle always reaches projection; later cycles cross budgets.
            case.limit_mode = (index / generator::FAMILIES) % LIMIT_MODES;
            let round = index / generator::FAMILIES;
            if round % 3 == 2 {
                generator.mutate(&mut case, round / 3 + index % generator::FAMILIES);
            }
            prepare(case)
        })
        .collect();
    sequence(&cases);
}

pub(super) fn smoke() {
    generated(36);
}

#[test]
fn fixed_seed_complete_render_is_bounded_and_deterministic() {
    smoke();
}

#[test]
fn complete_controls_reach_all_projections_and_limit_profiles() {
    let mut generator = generator::Generator::new();
    let mut reached = vec![String::new(); LIMIT_MODES];
    for index in 0..generator::FAMILIES {
        let mut case = generator.case(index);
        case.options = 7; // all metadata, allowed fallback, enabled frames
        let snapshots = replay(&case, &Renderer::new(), false);
        assert!(
            snapshots
                .iter()
                .all(|s| matches!(s, Snapshot::Artifact(..))),
            "{case:?}: {snapshots:?}"
        );
        if index == 5 {
            let Snapshot::Artifact(_, report) = &snapshots[2] else {
                unreachable!()
            };
            assert!(String::from_utf8_lossy(report).contains("pdf.raster-fallback"));
            case.options |= 16384;
            let forbidden = replay(&case, &Renderer::new(), false);
            let Snapshot::Failure(error, _) = &forbidden[2] else {
                panic!("fallback was not forbidden")
            };
            assert!(String::from_utf8_lossy(error).contains("pdf.raster-fallback-forbidden"));
            case.options = 7;
        }
        for (mode, reached) in reached.iter_mut().enumerate().skip(1) {
            case.limit_mode = mode;
            let first = replay(&case, &Renderer::new(), false);
            for snapshot in &first {
                match snapshot {
                    Snapshot::Artifact(_, report) => {
                        reached.push_str(&String::from_utf8_lossy(report))
                    }
                    Snapshot::Failure(error, report) => {
                        reached.push_str(&String::from_utf8_lossy(error));
                        reached.push_str(&String::from_utf8_lossy(report));
                    }
                    Snapshot::Rejected(_) => panic!("intact control rejected"),
                }
            }
            assert_eq!(first, replay(&case, &Renderer::new(), true));
        }
    }
    for (mode, code) in [
        (1, "resource.scene-bytes-exceeded"),
        (2, "resource.element-count-exceeded"),
        (3, "resource.total-geometry-exceeded"),
        (4, "resource.total-text-exceeded"),
        (5, "resource.diagnostic-count-exceeded"),
        (6, "resource.working-bytes-exceeded"),
        (7, "resource.output-bytes-exceeded"),
        (8, "resource.total-work-exceeded"),
        (9, "resource.raster-surface-exceeded"),
        (10, "resource.asset-bytes-exceeded"),
        (11, "resource.element-work-exceeded"),
        (12, "resource.optional-metadata-omitted"),
    ] {
        assert!(
            reached[mode].contains(code),
            "profile {mode} never reached {code}"
        );
    }
}

#[test]
fn subpixel_png_truncation_returns_an_atomic_encoding_failure() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#,
    )
    .unwrap();
    let request = RenderRequest::builder(OutputFormat::Png)
        .padding(0.0)
        .sizing(Sizing::Scale(0.5))
        .build()
        .unwrap();
    let error = Renderer::new().render(&scene, &request).unwrap_err();
    assert_eq!(error.code(), ErrorCode::ENCODE_PNG_FAILED);
    assert!(error.report().is_empty());
}

#[test]
fn metadata_omission_and_fatal_output_are_reached() {
    let mut case = generator::from_scene(
        "metadata-boundary",
        json!({"type":"excalidraw","version":2,
        "elements":[],"appState":{"future":"preserved original"},"files":{}}),
    );
    case.limit_mode = 12;
    for snapshot in replay(&case, &Renderer::new(), false) {
        let Snapshot::Artifact(_, report) = snapshot else {
            panic!("optional metadata should recover")
        };
        assert!(String::from_utf8_lossy(&report).contains("resource.optional-metadata-omitted"));
    }
    case.options = 0;
    case.limit_mode = 7;
    for snapshot in replay(&case, &Renderer::new(), false) {
        let Snapshot::Failure(error, _) = snapshot else {
            panic!("output exhaustion must fail atomically")
        };
        assert!(String::from_utf8_lossy(&error).contains("resource.output-bytes-exceeded"));
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "nightly complete-render expansion"]
fn nightly_complete_render_fuzz() {
    let count = std::env::var("EXCALIDRAW_RENDER_FUZZ_CASES")
        .map(|n| n.parse().unwrap())
        .unwrap_or(4096);
    assert!((1..=16_384).contains(&count));
    complete_controls_reach_all_projections_and_limit_profiles();
    metadata_omission_and_fatal_output_are_reached();
    generated(count);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "replay the complete captured JSON reproducer"]
fn replay_captured_complete_render() {
    let path = std::env::var("EXCALIDRAW_RENDER_FUZZ_REPLAY").expect("reproducer path");
    let bytes = std::fs::read(path).unwrap();
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Captured {
        Case(Case),
        Sequence(Vec<Case>),
    }
    let cases = match serde_json::from_slice(&bytes).unwrap() {
        Captured::Case(case) => vec![case],
        Captured::Sequence(cases) => cases,
    };
    assert!(!cases.is_empty() && cases.len() <= 16_384);
    sequence(&cases);
}
