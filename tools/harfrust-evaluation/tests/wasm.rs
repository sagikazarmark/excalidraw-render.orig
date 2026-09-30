//! Browser WASM must reproduce the native report exactly, both shapers.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn wasm_report_matches_native() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../expected-native.json")).unwrap();
    let actual = harfrust_evaluation::report();
    assert_eq!(actual["digests"], expected["digests"]);
    assert_eq!(actual, expected);
}
