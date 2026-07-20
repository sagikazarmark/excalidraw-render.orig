use excalidraw_render::{
    Artifact, CompatibilityDiagnostic, CompatibilityReport, ErrorDescriptor, RenderError,
    RenderOutcome,
};

#[test]
fn standalone_empty_report_has_the_byte_exact_schema_1_form() {
    let report = CompatibilityReport::default();

    assert_eq!(report.schema_version(), 1);
    assert!(report.is_empty());
    assert_eq!(report.len(), 0);
    assert_eq!(report.diagnostics(), &[]);
    assert_eq!(
        serde_json::to_vec(&report).expect("serialize empty compatibility report"),
        br#"{"schema_version":1,"diagnostics":[]}"#
    );
}

#[test]
fn owned_contract_values_have_the_required_public_traits() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_serialize<T: serde::Serialize>() {}

    assert_send_sync::<Artifact>();
    assert_send_sync::<CompatibilityDiagnostic>();
    assert_send_sync::<CompatibilityReport>();
    assert_send_sync::<RenderOutcome>();
    assert_send_sync::<RenderError>();

    assert_serialize::<CompatibilityDiagnostic>();
    assert_serialize::<CompatibilityReport>();
    assert_serialize::<ErrorDescriptor>();

    let _consume_outcome: fn(RenderOutcome) -> (Artifact, CompatibilityReport) =
        RenderOutcome::into_parts;
    let _consume_artifact: fn(Artifact) -> Vec<u8> = Artifact::into_bytes;
}
