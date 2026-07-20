use excalidraw_render::{
    ErrorCode, OutputFormat, RenderError, RenderErrorCategory, RenderRequest, Renderer,
    ResourceLimits, SceneDocument,
};
use serde_json::json;

const EMPTY_SCENE: &[u8] =
    br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;

#[test]
fn scene_byte_accounting_accepts_equality_and_reports_the_exact_crossing() {
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("empty scene is valid");
    let scene_bytes = u64::try_from(EMPTY_SCENE.len()).expect("fixture length fits u64");

    for configured in [scene_bytes, scene_bytes + 1] {
        let limits = ResourceLimits::builder()
            .max_scene_bytes(configured)
            .build()
            .expect("scene-byte boundary is valid");
        let request = request_with_limits(OutputFormat::Svg, limits);

        Renderer::new()
            .render(&scene, &request)
            .expect("equality and spare capacity are accepted");
    }

    let configured = scene_bytes - 1;
    let limits = ResourceLimits::builder()
        .max_scene_bytes(configured)
        .build()
        .expect("scene-byte crossing profile is valid");
    let request = request_with_limits(OutputFormat::Svg, limits);
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("one byte over the configured ceiling is fatal");

    assert_resource_failure(
        &error,
        ErrorCode::RESOURCE_SCENE_BYTES_EXCEEDED,
        "max_scene_bytes",
        configured,
        scene_bytes,
        "bytes",
        "",
    );
}

#[test]
fn every_source_element_occurrence_is_counted_before_rendering_support_is_considered() {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[null,false],"appState":{},"files":{}}"#,
    )
    .expect("scalar element occurrences remain admitted source content");
    let limits = ResourceLimits::builder()
        .max_elements(1)
        .build()
        .expect("element boundary is valid");
    let request = request_with_limits(OutputFormat::Svg, limits);

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the second source occurrence crosses the aggregate limit");

    assert_resource_failure(
        &error,
        ErrorCode::RESOURCE_ELEMENT_COUNT_EXCEEDED,
        "max_elements",
        1,
        2,
        "count",
        "/elements/1",
    );
}

#[test]
fn element_count_equality_and_spare_capacity_reach_the_next_rendering_phase() {
    let cases: &[(&[u8], u32)] = &[
        (
            br#"{"type":"excalidraw","version":2,"elements":[null],"appState":{},"files":{}}"#,
            2,
        ),
        (
            br#"{"type":"excalidraw","version":2,"elements":[null,false],"appState":{},"files":{}}"#,
            2,
        ),
    ];

    for (source, max_elements) in cases {
        let scene = SceneDocument::parse(source).expect("element occurrences are admitted");
        let limits = ResourceLimits::builder()
            .max_elements(*max_elements)
            .build()
            .expect("element boundary is valid");
        let request = request_with_limits(OutputFormat::Svg, limits);
        let error = Renderer::new()
            .render(&scene, &request)
            .expect_err("element projection is implemented by a later ticket");

        assert_eq!(error.code(), ErrorCode::RENDER_INTERNAL_FAILURE);
        assert_eq!(error.json_pointer(), "/elements");
    }
}

#[test]
fn element_count_precedes_total_work_at_the_same_source_occurrence() {
    let source =
        br#"{"type":"excalidraw","version":2,"elements":[null,null],"appState":{},"files":{}}"#;
    let scene = SceneDocument::parse(source).expect("element occurrences are admitted");
    let source_bytes = u64::try_from(source.len()).expect("fixture length fits u64");
    let total_before_second_element = source_bytes + 8 * 8;
    let max_total_work = total_before_second_element + 7;
    let limits = ResourceLimits::builder()
        .max_elements(1)
        .max_element_work_units(1)
        .max_asset_work_units(1)
        .max_total_work_units(max_total_work)
        .build()
        .expect("competing element limits are valid");
    let request = request_with_limits(OutputFormat::Svg, limits);
    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the second occurrence crosses both aggregate limits");

    assert_resource_failure(
        &error,
        ErrorCode::RESOURCE_ELEMENT_COUNT_EXCEEDED,
        "max_elements",
        1,
        2,
        "count",
        "/elements/1",
    );
}

#[test]
fn final_output_bytes_are_admitted_atomically_for_every_format() {
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let output_bytes = default_artifact_len(format);

        for configured in [output_bytes, output_bytes + 1] {
            let limits = ResourceLimits::builder()
                .max_output_bytes(configured)
                .build()
                .expect("output-byte boundary is valid");
            render_empty(format, limits).expect("equality and spare output capacity are accepted");
        }

        let configured = output_bytes - 1;
        let limits = ResourceLimits::builder()
            .max_output_bytes(configured)
            .build()
            .expect("output-byte crossing profile is valid");
        let error = render_empty(format, limits)
            .expect_err("one byte over the configured output ceiling is fatal");
        assert_resource_failure(
            &error,
            ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED,
            "max_output_bytes",
            configured,
            output_bytes,
            "bytes",
            "",
        );
    }
}

#[test]
fn canonical_peak_working_bytes_are_independent_of_physical_allocations() {
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let output_bytes = default_artifact_len(format);
        let peak = canonical_empty_working_byte_peak(format, output_bytes);

        for configured in [peak, peak + 1] {
            let limits = ResourceLimits::builder()
                .max_working_bytes(configured)
                .build()
                .expect("working-byte boundary is valid");
            render_empty(format, limits)
                .expect("equality and spare canonical working capacity are accepted");
        }

        let configured = peak - 1;
        let limits = ResourceLimits::builder()
            .max_working_bytes(configured)
            .build()
            .expect("working-byte crossing profile is valid");
        let error = render_empty(format, limits)
            .expect_err("one logical byte over the configured working ceiling is fatal");
        assert_resource_failure(
            &error,
            ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED,
            "max_working_bytes",
            configured,
            peak,
            "bytes",
            "",
        );
    }
}

#[test]
fn pdf_icc_profile_selection_is_live_through_artifact_retention() {
    let output_bytes = default_artifact_len(OutputFormat::Pdf);
    let peak = 4_096 + 1_024 + 128 + 256 + 1_048_576 + 512 + logical_aligned_bytes(output_bytes);

    let equality = ResourceLimits::builder()
        .max_working_bytes(peak)
        .build()
        .expect("PDF ICC selection equality limit is valid");
    render_empty(OutputFormat::Pdf, equality)
        .expect("the ICC selection record fits at exact equality");

    let crossing = ResourceLimits::builder()
        .max_working_bytes(peak - 1)
        .build()
        .expect("PDF ICC selection crossing limit is valid");
    let error = render_empty(OutputFormat::Pdf, crossing)
        .expect_err("the retained ICC selection contributes to the working-byte peak");
    assert_resource_failure(
        &error,
        ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED,
        "max_working_bytes",
        peak - 1,
        peak,
        "bytes",
        "",
    );
}

#[test]
fn canonical_total_work_is_charged_through_artifact_retention() {
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let output_bytes = default_artifact_len(format);
        let total_work = canonical_empty_total_work(format, output_bytes);

        for configured in [total_work, total_work + 1] {
            let limits = ResourceLimits::builder()
                .max_element_work_units(1)
                .max_asset_work_units(1)
                .max_total_work_units(configured)
                .build()
                .expect("total-work boundary is valid");
            render_empty(format, limits)
                .expect("equality and spare canonical work capacity are accepted");
        }

        let configured = total_work - 1;
        let limits = ResourceLimits::builder()
            .max_element_work_units(1)
            .max_asset_work_units(1)
            .max_total_work_units(configured)
            .build()
            .expect("total-work crossing profile is valid");
        let error = render_empty(format, limits)
            .expect_err("one work unit over the configured total ceiling is fatal");
        assert_resource_failure(
            &error,
            ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED,
            "max_total_work_units",
            configured,
            total_work,
            "work_units",
            "",
        );
    }
}

#[test]
fn competing_final_aggregate_crossings_use_normative_precedence() {
    let format = OutputFormat::Svg;
    let output_bytes = default_artifact_len(format);
    let peak = canonical_empty_working_byte_peak(format, output_bytes);
    let total_work = canonical_empty_total_work(format, output_bytes);
    let cases = [
        (
            peak - 1,
            output_bytes - 1,
            total_work - 1,
            ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED,
            "max_working_bytes",
            peak - 1,
            peak,
            "bytes",
        ),
        (
            peak,
            output_bytes - 1,
            total_work - 1,
            ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED,
            "max_output_bytes",
            output_bytes - 1,
            output_bytes,
            "bytes",
        ),
        (
            peak,
            output_bytes,
            total_work - 1,
            ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED,
            "max_total_work_units",
            total_work - 1,
            total_work,
            "work_units",
        ),
    ];

    for (max_working, max_output, max_work, code, field, configured, attempted, value_kind) in cases
    {
        let limits = ResourceLimits::builder()
            .max_working_bytes(max_working)
            .max_output_bytes(max_output)
            .max_element_work_units(1)
            .max_asset_work_units(1)
            .max_total_work_units(max_work)
            .build()
            .expect("competing-crossing profile is valid");
        let error = render_empty(format, limits).expect_err("the final transaction is rejected");
        assert_resource_failure(&error, code, field, configured, attempted, value_kind, "");
    }
}

#[test]
fn scene_bytes_precede_total_work_at_the_same_source_event() {
    let attempted = u64::try_from(EMPTY_SCENE.len()).expect("fixture length fits u64");
    let configured = attempted - 1;
    let limits = ResourceLimits::builder()
        .max_scene_bytes(configured)
        .max_element_work_units(1)
        .max_asset_work_units(1)
        .max_total_work_units(configured)
        .build()
        .expect("competing scene limits are valid");
    let error = render_empty(OutputFormat::Svg, limits)
        .expect_err("scene bytes and source-octet work both cross");

    assert_resource_failure(
        &error,
        ErrorCode::RESOURCE_SCENE_BYTES_EXCEEDED,
        "max_scene_bytes",
        configured,
        attempted,
        "bytes",
        "",
    );
}

fn request_with_limits(format: OutputFormat, limits: ResourceLimits) -> RenderRequest {
    RenderRequest::builder(format)
        .resource_limits(limits)
        .build()
        .expect("resource-limited request is valid")
}

fn render_empty(
    format: OutputFormat,
    limits: ResourceLimits,
) -> Result<excalidraw_render::RenderOutcome, RenderError> {
    let scene = SceneDocument::parse(EMPTY_SCENE).expect("empty scene is valid");
    let request = request_with_limits(format, limits);
    Renderer::new().render(&scene, &request)
}

fn default_artifact_len(format: OutputFormat) -> u64 {
    let outcome = render_empty(format, ResourceLimits::default())
        .expect("default empty-scene render succeeds");
    u64::try_from(outcome.artifact().bytes().len()).expect("artifact length fits u64")
}

fn canonical_empty_working_byte_peak(format: OutputFormat, output_bytes: u64) -> u64 {
    let session_and_plan = 4_096 + 1_024 + 128;
    let workspace_and_surface = match format {
        OutputFormat::Svg => 256 * 1_024,
        OutputFormat::Png => (256 + 4 * 20 * 20) + (2 * 1_048_576 + 16 * 20),
        OutputFormat::Pdf => 256 + 1_048_576,
        _ => unreachable!("all revision-1 formats are covered"),
    };
    session_and_plan + workspace_and_surface + 512 + logical_aligned_bytes(output_bytes)
}

fn canonical_empty_total_work(format: OutputFormat, output_bytes: u64) -> u64 {
    let source_and_json_and_plan = 72 + 11 * 8 + 8;
    let format_work = match format {
        OutputFormat::Svg | OutputFormat::Pdf => 0,
        OutputFormat::Png => {
            let rgba_bytes = 4 * 20 * 20;
            let solid_background_samples = 4 * 20 * 20;
            solid_background_samples + rgba_bytes + rgba_bytes
        }
        _ => unreachable!("all revision-1 formats are covered"),
    };
    source_and_json_and_plan + format_work + output_bytes
}

fn logical_aligned_bytes(bytes: u64) -> u64 {
    bytes.div_ceil(64) * 64
}

fn assert_resource_failure(
    error: &RenderError,
    code: ErrorCode,
    field: &str,
    configured: u64,
    attempted: u64,
    value_kind: &str,
    pointer: &str,
) {
    assert_eq!(error.category(), RenderErrorCategory::ResourceLimit);
    assert_eq!(error.code(), code);
    assert_eq!(error.json_pointer(), pointer);
    assert_eq!(error.element_id(), None);
    assert!(error.report().is_empty());

    let descriptor = serde_json::to_value(error.descriptor()).expect("descriptor serializes");
    assert_eq!(
        descriptor["details"],
        json!({
            "kind": "resource_limit",
            "field": field,
            "configured": configured,
            "value_kind": value_kind,
            "measurement": {"kind": "attempted", "value": attempted},
            "recovery": null,
        })
    );
}
