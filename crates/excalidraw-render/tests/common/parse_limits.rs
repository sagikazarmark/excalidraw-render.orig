//! The shape of one parser-limit rejection, asserted the same way by the scene
//! admission tests and by the limits tier that builds ceiling-sized scenes.

use excalidraw_render::{ErrorCode, ErrorDetails, Measurement, SceneParseError, ValueKind};

pub fn assert_parse_limit(
    error: SceneParseError,
    expected_code: ErrorCode,
    expected_pointer: Option<&str>,
    expected_field: &str,
    expected_configured: u64,
    expected_value_kind: ValueKind,
    expected_measurement: Measurement,
) {
    assert_eq!(error.code(), expected_code);
    assert_eq!(error.json_pointer(), expected_pointer);
    match error.details() {
        Some(ErrorDetails::ParseLimit {
            field,
            configured,
            value_kind,
            measurement,
            ..
        }) => {
            assert_eq!(*field, expected_field);
            assert_eq!(*configured, expected_configured);
            assert_eq!(*value_kind, expected_value_kind);
            assert_eq!(*measurement, expected_measurement);
        }
        details => panic!("expected parse-limit details, got {details:?}"),
    }
}
