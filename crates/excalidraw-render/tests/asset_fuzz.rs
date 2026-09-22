//! Public-facade codec and nested-SVG fuzz target; ordinary smoke plus nightly expansion.
#[path = "asset_fuzz/mod.rs"]
mod asset_fuzz;
#[path = "common/codec_fixtures.rs"]
mod codec_fixtures;
#[allow(dead_code)]
#[path = "common/png_color_reference.rs"]
mod png_color_reference;
#[allow(dead_code)]
#[path = "common/png_icc_safety.rs"]
mod png_icc_safety;
#[allow(dead_code)]
#[path = "common/png_stage4.rs"]
mod png_stage4;
#[allow(dead_code)]
#[path = "common/png_stage4_reference.rs"]
mod png_stage4_reference;
#[path = "common/svg_resource_inspect.rs"]
mod svg_resource_inspect;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen_test::wasm_bindgen_test;
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

    #[wasm_bindgen_test]
    fn asset_fuzz_smoke_in_wasm() {
        crate::asset_fuzz::assert_asset_fuzz_smoke();
    }
}
