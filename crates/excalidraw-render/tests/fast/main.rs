//! The fast tier: every integration test of the renderer that needs neither
//! external tooling, nor official evidence to compare against, nor a
//! ceiling-sized input, one module per former test binary. The tests that do
//! are their own binaries under `tests/`, named by tier: `parity_*`, `limits_*`
//! and `harness_*`. One binary here instead of forty links the renderer and its
//! dependencies once, and `cargo nextest` still runs every test in its own
//! process.
//!
//! Run one module's tests with `cargo nextest run -E 'test(/^arrow_render::/)'`
//! or `cargo test --test fast arrow_render::`.

#[cfg(target_arch = "wasm32")]
mod browser {
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);
}

#[path = "../common/mod.rs"]
mod common;

mod admission_identity_contract;
mod arrow_render;
mod artifact_metadata;
mod bound_text_render;
mod compatibility_fallback_render;
mod compatibility_identity;
mod compatibility_recovery_contract;
mod coordinate_recovery;
mod dark_mode_frame_and_image_render;
mod diagnostic_contract;
mod duplicate_identity_contract;
mod emoji_fallback_render;
mod empty_scene_render;
mod empty_text_line_render;
mod excalifont_text_render;
mod final_surface_contract;
mod font_catalog_render;
mod frame_clip_accounting;
mod frame_export;
mod frame_field_contract;
mod frame_render;
mod freedraw_render;
mod geometry_style_contract;
mod gif_control_contract;
mod gif_first_frame_contract;
mod gif_lzw_contract;
mod helvetica_text_render;
mod image_corner_clip;
mod image_edge_coverage;
mod image_field_contract;
mod image_resampling;
mod image_viewport_rounding;
mod jpeg_color_contract;
mod jpeg_contract;
mod jpeg_exif_contract;
mod jpeg_gain_map_contract;
mod jpeg_icc_contract;
mod license_bundle;
mod linear_rotation_render;
mod missing_glyph_render;
mod nested_svg_asset_render;
mod normalization;
mod outcome_contract;
mod outline_shape_render;
mod pdf_backend_qualification;
mod pdf_raster_fallback_contract;
mod pdf_renderer_expectations;
mod png_color_contract;
mod png_precision_contract;
mod png_profile_contract;
mod png_stage4_contract;
mod point_stroke_contract;
mod rectangle_pdf_render;
mod rectangle_png_render;
mod rectangle_svg_render;
mod registered_case_determinism;
mod relationship_field_contract;
mod relationships;
mod render_request;
mod resource_accounting;
mod resource_diagnostic_contract;
mod rough_fillers_dots_scene;
mod rough_style_render;
mod scene_admission;
mod scene_bounds_render;
mod scene_surface_policy;
mod scratch_dir;
mod sketch_fill_pdf_split;
mod soft_hyphen_render;
mod stage4_interaction_contract;
mod static_bmp_asset_render;
mod static_gif_asset_render;
mod static_ico_asset_render;
mod static_jpeg_asset_render;
mod static_png_asset_render;
mod static_webp_asset_render;
mod surface_metadata_contract;
mod svg_link_diagnostic_contract;
mod text_field_contract;
mod text_line_endings_render;
mod webp_sample_contract;
mod whole_scene_controls;
