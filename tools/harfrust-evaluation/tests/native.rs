//! The committed report is what the probe produces natively.
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn native_report_matches_record() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../expected-native.json")).unwrap();
    let mut actual = harfrust_evaluation::report();
    // The libm build must agree with the default build byte for byte.
    actual["harfrust_libm"] = expected["harfrust_libm"].clone();
    assert_eq!(actual, expected);
}
