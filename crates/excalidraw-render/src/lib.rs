//! Pure-Rust rendering of Excalidraw scene documents.
//!
//! Scene and frame exports to SVG, PNG, and vector-first PDF are produced
//! against a compatibility revision identifying the rendering profile.
//! In 0.x, output and diagnostics may change between reviewed releases;
//! pin the renderer version and dependency lockfile for reproducible output.
//! Linux x86-64 GNU is release-qualified; browser WASM is experimental.

mod catalog;
#[cfg(test)]
mod cms_failure_contract;
#[cfg(test)]
#[path = "../tests/common/codec_fixtures.rs"]
mod codec_fixtures;
mod color;
mod compatibility;
mod css_syntax;
mod diagnostic_registry;
mod font_metadata;
mod fonts;
mod frame;
#[cfg(test)]
mod metadata_failure_contract;
mod normalization;
mod outcome;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/common/png_color_reference.rs"]
mod png_color_reference;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/common/png_icc_safety.rs"]
mod png_icc_safety;
#[cfg(test)]
mod png_projection_contract;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/common/png_stage4.rs"]
mod png_stage4;
#[cfg(all(test, target_arch = "wasm32"))]
#[path = "../tests/common/png_stage4_composition.rs"]
mod png_stage4_composition;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/common/png_stage4_reference.rs"]
mod png_stage4_reference;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/common/png_stage4_review.rs"]
mod png_stage4_review;
mod portable_math;
mod relationships;
mod render;
mod request;
mod resource_accounting;
mod resource_limits;
mod scene;
#[cfg(test)]
mod scene_fuzz;
mod scene_surface;
#[cfg(test)]
mod stage4_diagnostic_contract;

pub use compatibility::{
    AssetChainEntry, CompatibilityRevision, ErrorCode, ErrorDescriptor, ErrorDetails, ErrorFamily,
    FiniteValue, LogicalBounds, Measurement, MetadataComponent, MetadataOperation, NestedPath,
    NestedPathSegment, NestedSegmentKind, Recovery, ReferenceProfileIdentity, Renderer,
    ResourceMeasurement, ResourceValue, SvgFontSubsetStage, UnsupportedRevisionError, ValueKind,
    DEFAULT_COMPATIBILITY_REVISION,
};
pub use frame::{
    FrameBatchError, FrameBatchErrorKind, FrameKind, FrameRef, FrameRefs, FrameRenderCursor,
    FrameRenderEntry,
};
pub use outcome::{
    Artifact, CompatibilityDiagnostic, CompatibilityReport, DiagnosticCode, DiagnosticEffect,
    DiagnosticSeverity, RenderError, RenderErrorCategory, RenderOutcome,
};
pub use request::{
    ColorMode, FrameOptions, FrameRenderRequest, FrameRenderRequestBuilder, MetadataOptions,
    OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest, RenderRequestBuilder, RequestError,
    Sizing, SupplementalAsset, SupplementalAssets, SupplementalAssetsBuilder,
};
pub use resource_limits::{ResourceLimits, ResourceLimitsBuilder};
pub use scene::{SceneDocument, SceneParseError};

#[cfg(all(test, target_arch = "wasm32"))]
pub mod wasm_tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

    #[wasm_bindgen_test]
    fn png_sdr_color_reference_in_wasm() {
        png_color_cases_in_wasm(crate::png_color_reference::evidence());
    }

    #[wasm_bindgen_test]
    fn png_gain_map_reference_in_wasm() {
        png_color_cases_in_wasm(crate::png_color_reference::gain_map_evidence());
    }

    #[wasm_bindgen_test]
    fn png_stage4_icc_reference_in_wasm() {
        png_color_cases_in_wasm(crate::png_stage4_reference::evidence());
    }
    #[wasm_bindgen_test]
    fn cms_allocation_failures_in_wasm() {
        crate::cms_failure_contract::cms_out_of_memory_is_fatal_at_every_operation_boundary();
    }
    #[wasm_bindgen_test]
    fn png_stage4_profile_composition_in_wasm() {
        png_color_cases_in_wasm(crate::png_stage4_composition::evidence());
    }
    #[wasm_bindgen_test]
    fn png_stage4_review_regressions_in_wasm() {
        png_stage4_bytes(crate::png_icc_safety::headers());
        png_color_cases_in_wasm(crate::png_icc_safety::sampled_evidence());
        png_stage4_bytes(crate::png_stage4_review::adam7());
        png_stage4_bytes(crate::png_stage4_review::icc_extents());
        png_stage4_bytes(crate::png_stage4_review::lower_keys());
        png_color_cases_in_wasm(crate::png_stage4_review::gray_lut_equivalence());
    }
    #[wasm_bindgen_test]
    fn png_profile_projection_limits_in_wasm() {
        crate::png_projection_contract::profile_limits(Some(0));
    }
    #[wasm_bindgen_test]
    fn png_profile_rounded_limits_in_wasm() {
        crate::png_projection_contract::profile_limits(Some(1));
    }
    #[wasm_bindgen_test]
    fn png_profile_frame_mask_limits_in_wasm() {
        crate::png_projection_contract::profile_limits(Some(2));
    }

    #[wasm_bindgen_test]
    fn png_stage4_byte_contracts_in_wasm() {
        use crate::png_stage4 as v;
        for rows in [
            v::orientations(),
            v::invalid_orientation(),
            v::chunk_cases(),
            v::scanline_cases(),
            v::palette_cases(),
            v::apng_controls(),
            v::apng_malformed(),
            v::invalid_cicp(),
            v::large_streams(),
            v::composition(&[(*b"gAMA", 100000_u32.to_be_bytes().to_vec())], true),
            v::hdr_composition(16),
            v::hdr_composition(18),
        ] {
            png_stage4_bytes(rows);
        }
    }

    #[wasm_bindgen_test]
    fn png_stage4_ihdr_contracts_in_wasm() {
        png_stage4_bytes(crate::png_stage4::ihdr_cases());
    }

    fn png_stage4_bytes(rows: Vec<crate::png_stage4::Case>) {
        use crate::png_stage4::{self as v, Outcome};
        for row in rows {
            let source = v::scene(&row);
            let scene = crate::SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
            let request = crate::RenderRequest::builder(crate::OutputFormat::Png)
                .padding(0.0)
                .include_background(false)
                .build()
                .unwrap();
            let first = crate::Renderer::new().render(&scene, &request).unwrap();
            let second = crate::Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(first.report(), second.report());
            v::report(&row, "png", serde_json::to_value(first.report()).unwrap());
            if matches!(row.outcome, Outcome::Accepted(_)) {
                let actual = crate::png_color_reference::png(first.artifact().bytes());
                assert_eq!(
                    (actual.width() as usize, actual.height() as usize),
                    (row.width, row.height)
                );
                assert_eq!(actual.rgba(), row.pixels, "{}", row.id);
            } else {
                let mut source = source;
                source["files"] = serde_json::json!({});
                let control =
                    crate::SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
                assert_eq!(
                    first.artifact().bytes(),
                    crate::Renderer::new()
                        .render(&control, &request)
                        .unwrap()
                        .artifact()
                        .bytes()
                );
            }
        }
    }

    fn png_color_cases_in_wasm(evidence: serde_json::Value) {
        for row in evidence["cases"].as_array().unwrap() {
            let scene =
                crate::SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
            let request = crate::RenderRequest::builder(crate::OutputFormat::Png)
                .padding(0.0)
                .include_background(false)
                .build()
                .unwrap();
            let first = crate::Renderer::new().render(&scene, &request).unwrap();
            let second = crate::Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(
                serde_json::to_vec(first.report()).unwrap(),
                serde_json::to_vec(second.report()).unwrap()
            );
            if row["group"] == "composition" {
                let late = crate::SceneDocument::parse(
                    &serde_json::to_vec(&crate::png_stage4_review::late_scene(row)).unwrap(),
                )
                .unwrap();
                let late = crate::Renderer::new().render(&late, &request).unwrap();
                assert_eq!(first.artifact().bytes(), late.artifact().bytes());
                assert_eq!(first.report(), late.report());
            }
            if row["group"] == "composition" {
                crate::png_stage4_composition::report(
                    row,
                    "png",
                    serde_json::to_value(first.report()).unwrap(),
                );
            } else {
                crate::png_color_reference::report(
                    row,
                    "png",
                    serde_json::to_value(first.report()).unwrap(),
                );
            }
            if let Some(equivalent) = crate::png_color_reference::bt1886_equivalent(&evidence, row)
                .or_else(|| crate::png_color_reference::ignored_cicp_equivalent(&evidence, row))
                .or_else(|| crate::png_color_reference::gain_map_equivalent(&evidence, row))
            {
                let scene =
                    crate::SceneDocument::parse(&serde_json::to_vec(&equivalent["scene"]).unwrap())
                        .unwrap();
                let control = crate::Renderer::new().render(&scene, &request).unwrap();
                assert_eq!(
                    first.artifact().bytes(),
                    control.artifact().bytes(),
                    "{} exact metadata equivalence",
                    row["id"]
                );
                assert_eq!(first.report(), control.report());
            }
            if crate::png_color_reference::recovery(row, "png").is_some() {
                let mut control = row["scene"].clone();
                control["files"] = serde_json::json!({});
                let control =
                    crate::SceneDocument::parse(&serde_json::to_vec(&control).unwrap()).unwrap();
                assert_eq!(
                    first.artifact().bytes(),
                    crate::Renderer::new()
                        .render(&control, &request)
                        .unwrap()
                        .artifact()
                        .bytes()
                );
            } else {
                crate::png_stage4_reference::anchor(
                    row,
                    crate::png_color_reference::png(first.artifact().bytes()).rgba(),
                );
                crate::png_color_reference::appearance(
                    row,
                    crate::png_color_reference::png(first.artifact().bytes()),
                    false,
                );
            }
        }
    }

    #[wasm_bindgen_test]
    fn png_precision_reference_in_wasm() {
        use base64::Engine as _;
        use sha2::{Digest, Sha256};
        let bytes = include_bytes!("../../../tests/oracle/png-precision/reference.json");
        assert_eq!(
            crate::resource_accounting::lower_hex(&Sha256::digest(bytes)),
            "8400494ec5f6e744b00e175db6931aff33c2a659dc57c3b3ab62646da0cf83e5"
        );
        let evidence: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let decode_png = |bytes: &[u8]| {
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .unwrap();
            let mut rgba = vec![0; reader.output_buffer_size().expect("PNG buffer fits")];
            let info = reader.next_frame(&mut rgba).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba);
            assert_eq!(info.bit_depth, png::BitDepth::Eight);
            rgba.truncate(info.buffer_size());
            (info.width, info.height, rgba)
        };
        for row in evidence["cases"].as_array().unwrap() {
            let scene =
                crate::SceneDocument::parse(&serde_json::to_vec(&row["scene"]).unwrap()).unwrap();
            let request = crate::RenderRequest::builder(crate::OutputFormat::Png)
                .padding(0.0)
                .include_background(row["options"]["exportBackground"].as_bool().unwrap())
                .build()
                .unwrap();
            let first = crate::Renderer::new().render(&scene, &request).unwrap();
            let second = crate::Renderer::new().render(&scene, &request).unwrap();
            assert_eq!(first.artifact().bytes(), second.artifact().bytes());
            assert_eq!(first.report(), second.report());
            assert_eq!(
                serde_json::to_value(first.report()).unwrap(),
                serde_json::json!({"schema_version":1,"diagnostics":[]})
            );
            let expected = base64::engine::general_purpose::STANDARD
                .decode(row["png_base64"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                decode_png(first.artifact().bytes()),
                decode_png(&expected),
                "{}",
                row["id"]
            );
        }
    }

    #[wasm_bindgen_test]
    fn complete_font_subset_qualification_corpus_in_wasm() {
        crate::render::assert_complete_font_subset_qualification_corpus();
    }

    #[wasm_bindgen_test]
    fn font_subset_fuzz_regression_corpus_in_wasm() {
        crate::render::assert_font_subset_fuzz_regression_corpus();
    }

    #[wasm_bindgen_test]
    fn font_subset_fuzz_fixed_seed_smoke_in_wasm() {
        crate::render::assert_font_subset_fuzz_smoke();
    }

    #[wasm_bindgen_test]
    fn font_subset_resource_trace_matrix_in_wasm() {
        crate::render::assert_font_subset_resource_trace_matrix();
    }

    #[wasm_bindgen_test]
    fn scene_fuzz_regression_corpus_in_wasm() {
        crate::scene_fuzz::assert_scene_fuzz_regression_corpus();
    }

    #[wasm_bindgen_test]
    fn scene_fuzz_fixed_seed_smoke_in_wasm() {
        crate::scene_fuzz::assert_scene_fuzz_smoke();
    }
}

/// Returns the complete package-wide license and attribution bundle.
pub const fn license_bundle() -> &'static str {
    include_str!("../LICENSE-BUNDLE.txt")
}
