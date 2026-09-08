//! The fast tier: every integration test of the renderer that needs neither
//! external tooling, nor a registry of official evidence, nor a ceiling-sized
//! input, one module per former test binary. The tests that do are their own
//! binaries under `tests/`, named by tier: `parity_*`, `limits_*` and
//! `harness_*`. One binary here instead of forty links the renderer and its
//! dependencies once, and `cargo nextest` still runs every test in its own
//! process.
//!
//! Run one module's tests with `cargo nextest run -E 'test(/^arrow_render::/)'`
//! or `cargo test --test fast arrow_render::`.

#[path = "../common/mod.rs"]
mod common;

mod arrow_render;
mod artifact_metadata;
mod bound_text_render;
mod compatibility_fallback_render;
mod compatibility_identity;
mod dark_mode_frame_and_image_render;
mod emoji_fallback_render;
mod empty_scene_render;
mod empty_text_line_render;
mod excalifont_text_render;
mod font_catalog_render;
mod frame_clip_accounting;
mod frame_export;
mod frame_render;
mod freedraw_render;
mod helvetica_text_render;
mod image_corner_clip;
mod image_viewport_rounding;
mod legacy_line_height_render;
mod license_bundle;
mod missing_glyph_render;
mod nested_svg_asset_render;
mod normalization;
mod outcome_contract;
mod outline_shape_render;
mod pdf_backend_qualification;
mod pdf_renderer_expectations;
mod rectangle_pdf_render;
mod rectangle_png_render;
mod rectangle_svg_render;
mod relationships;
mod render_request;
mod resource_accounting;
mod rough_style_render;
mod scene_admission;
mod scene_surface_policy;
mod scratch_dir;
mod static_bmp_asset_render;
mod static_gif_asset_render;
mod static_ico_asset_render;
mod static_jpeg_asset_render;
mod static_png_asset_render;
mod static_webp_asset_render;
mod text_line_endings_render;
mod whole_scene_controls;
