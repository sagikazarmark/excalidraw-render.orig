//! Complete rendering properties at the public facade confirmed in #30/#83.
#[allow(dead_code)]
#[path = "common/pdf_inspect.rs"]
mod pdf_inspect;
#[path = "render_fuzz/mod.rs"]
mod render_fuzz;
#[path = "common/svg_resource_inspect.rs"]
mod svg_resource_inspect;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen_test::wasm_bindgen_test;
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

    #[wasm_bindgen_test]
    fn complete_render_fuzz_smoke_in_wasm() {
        crate::render_fuzz::smoke();
    }
}
