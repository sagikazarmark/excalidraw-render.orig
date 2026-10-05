use excalidraw_render::{
    DiagnosticDetails, ErrorCode, OutputFormat, RenderError, RenderErrorCategory, RenderRequest,
    Renderer, ResourceLimits, ResourceMeasurement, ResourceValue, SceneDocument,
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
        let outcome = Renderer::new()
            .render(&scene, &request)
            .expect("diagnosed malformed occurrences retain a meaningful artifact");

        assert!(!outcome.report().is_empty());
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
fn required_png_surface_is_admitted_before_projection_and_fallbacks() {
    let required_pixels = 20 * 20;

    let accepted = ResourceLimits::builder()
        .max_raster_surface_pixels(required_pixels)
        .max_total_rasterized_pixels(required_pixels)
        .build()
        .expect("required-raster equality limits are valid");
    render_empty(OutputFormat::Png, accepted)
        .expect("the required main raster is accepted at exact equality");

    let configured = required_pixels - 1;
    let rejected = ResourceLimits::builder()
        .max_raster_surface_pixels(configured)
        .max_total_rasterized_pixels(configured)
        .build()
        .expect("required-raster crossing limits are valid");
    let error = render_empty(OutputFormat::Png, rejected)
        .expect_err("the required main raster is rejected before projection");

    assert_resource_failure(
        &error,
        ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED,
        "max_raster_surface_pixels",
        configured,
        required_pixels,
        "pixels",
        "",
    );
    // The human message carries what the details carry: the field, the
    // attempted value and the configured ceiling.
    let message = format!(
        "required PNG raster surface exceeds its configured limit: \
         needs {required_pixels} pixels, but max_raster_surface_pixels is {configured}"
    );
    assert_eq!(error.to_string(), message);
    assert_eq!(error.descriptor().message(), message);
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

#[test]
fn locally_rejected_geometry_still_participates_in_the_aggregate_limit() {
    let source = br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"first","type":"rectangle","x":0,"y":0,"width":100,"height":100,"strokeColor":"#000000","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":1,"strokeStyle":"solid","roughness":1,"opacity":100,"angle":0,"seed":1,"updated":1},
            {"id":"second","type":"rectangle","x":120,"y":0,"width":100,"height":100,"strokeColor":"#000000","backgroundColor":"transparent","fillStyle":"solid","strokeWidth":1,"strokeStyle":"solid","roughness":1,"opacity":100,"angle":0,"seed":1,"updated":1}
        ],
        "appState":{},"files":{}
    }"##;
    let scene = SceneDocument::parse(source).expect("two rectangles are valid");
    let limits = ResourceLimits::builder()
        .max_geometry_segments_per_element(15)
        .max_total_geometry_segments(31)
        .build()
        .expect("local and aggregate geometry limits are valid");
    let request = request_with_limits(OutputFormat::Svg, limits);

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the second local rejection crosses the aggregate geometry limit");

    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_GEOMETRY_EXCEEDED);
    assert_eq!(error.element_id(), Some("second"));
    assert_eq!(error.json_pointer(), "/elements/1");
    assert_eq!(error.report().diagnostics().len(), 1);
    assert_eq!(
        error.report().diagnostics()[0].code().as_str(),
        "resource.element-complexity-replaced"
    );
}

#[test]
fn locally_rejected_text_still_participates_in_the_aggregate_limit() {
    let source = br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"first","type":"text","x":0,"y":0,"width":100,"height":30,"strokeColor":"#000000","opacity":100,"fontSize":20,"fontFamily":5,"text":"ab","textAlign":"left","verticalAlign":"top","lineHeight":1.2,"updated":1},
            {"id":"second","type":"text","x":0,"y":40,"width":100,"height":30,"strokeColor":"#000000","opacity":100,"fontSize":20,"fontFamily":5,"text":"cd","textAlign":"left","verticalAlign":"top","lineHeight":1.2,"updated":1}
        ],
        "appState":{},"files":{}
    }"##;
    let scene = SceneDocument::parse(source).expect("two text elements are valid");
    let limits = ResourceLimits::builder()
        .max_text_scalars_per_element(1)
        .max_total_text_scalars(3)
        .build()
        .expect("local and aggregate text limits are valid");
    let request = request_with_limits(OutputFormat::Svg, limits);

    let error = Renderer::new()
        .render(&scene, &request)
        .expect_err("the second local rejection crosses the aggregate text limit");

    assert_eq!(error.code(), ErrorCode::RESOURCE_TOTAL_TEXT_EXCEEDED);
    assert_eq!(error.element_id(), Some("second"));
    assert_eq!(error.json_pointer(), "/elements/1");
    assert_eq!(error.report().diagnostics().len(), 1);
    assert_eq!(
        error.report().diagnostics()[0].code().as_str(),
        "resource.element-complexity-replaced"
    );
}

#[test]
fn font_fallback_probes_charge_the_exact_vector_for_every_fallback_shape() {
    // Spec 8.3.1 and 13.2: a text element's work is 32 units for each
    // normalized scalar, shaped glyph, line-break step, font fallback probe,
    // and glyph-outline command. A shaped scalar the whole-text face maps pays
    // no probe; otherwise it pays one probe per official font chunk of its
    // family whose declared range covers it, one per Xiaolai chunk likewise
    // when the CJK subset trigger selected Xiaolai, one for the Noto Emoji
    // consultation, and one for the Liberation Sans replacement glyph. The
    // scalar, glyph, and line-step rows are prepaid before shaping; the probe
    // and outline rows are the remainder charged at layout. The outline
    // count is that element's own geometry crossing; every other row is a
    // literal read off the text and that rule.
    let cases: [(&str, u64, u64, &str); 5] = [
        ("A", 1, 0, "Excalifont's basic-Latin face maps it: no probe"),
        (
            "\u{20a9}",
            1,
            2,
            "untriggered missing scalar: no Excalifont chunk declares the won sign, \
             Noto Emoji lacks it, the replacement glyph serves it",
        ),
        (
            "\u{4e2d}",
            1,
            1,
            "triggered Xiaolai scalar: the one Xiaolai chunk declaring the ideograph",
        ),
        ("\u{1f600}", 1, 1, "emoji: the Noto Emoji consultation"),
        (
            "\u{1f600}\u{20a9}",
            2,
            1 + 2,
            "a selected Noto Emoji face is consulted once for the won sign, as an \
             unselected one is",
        ),
    ];
    for (text, glyphs, probes, why) in cases {
        let scene = excalifont_scene(&[("text", text)]);
        let scalars = u64::try_from(text.chars().count()).expect("small");
        let line_break_steps = 1;
        let prepaid = 32 * (scalars + glyphs + line_break_steps);
        let (outline_commands, work_units) = text_element_work_vector(&scene, "text", prepaid);
        assert_eq!(
            work_units,
            prepaid + 32 * (probes + outline_commands),
            "{text:?}: {why}"
        );
    }
}

#[test]
fn font_fallback_probes_do_not_depend_on_which_occurrences_came_before() {
    // Spec 8.3.1 and 13.2: once the family's whole-text face is fixed, the
    // faces already serving the target change neither the probe count nor
    // the order, so the won sign pays the same two probes whether or not an
    // earlier text occurrence already selected Noto Emoji.
    let alone = excalifont_scene(&[("won", "\u{20a9}")]);
    let after_emoji = excalifont_scene(&[("emoji", "\u{1f600}"), ("won", "\u{20a9}")]);
    let prepaid = 32 * (1 + 1 + 1);
    let (alone_outline, alone_work) = text_element_work_vector(&alone, "won", prepaid);
    let (after_outline, after_work) = text_element_work_vector(&after_emoji, "won", prepaid);
    assert_eq!(alone_outline, after_outline, "the same replacement glyph");
    assert_eq!(alone_work, prepaid + 32 * (2 + alone_outline));
    assert_eq!(after_work, alone_work);
}

#[test]
fn a_family_whole_text_face_is_fixed_by_its_first_live_occurrence() {
    // Spec 8.3.1: a resolved family's whole-text face is fixed once per
    // target, by the family's first live occurrence, and every later
    // occurrence shapes with it. Excalifont's basic-Latin chunk declares and
    // maps `A` but not U+0100; its Latin-extended chunk declares and maps
    // U+0100 but not `A`. Alone, each scalar is mapped by the face its own
    // text selects and pays no probe; after an occurrence that fixed the
    // other chunk, it pays one probe for the one Excalifont chunk declaring
    // it, whichever order the two occurrences take.
    let prepaid = 32 * (1 + 1 + 1);
    for (first, second) in [("A", "\u{100}"), ("\u{100}", "A")] {
        let alone = excalifont_scene(&[("second", second)]);
        let after_first = excalifont_scene(&[("first", first), ("second", second)]);
        let (alone_outline, alone_work) = text_element_work_vector(&alone, "second", prepaid);
        let (after_outline, after_work) = text_element_work_vector(&after_first, "second", prepaid);
        assert_eq!(
            alone_outline, after_outline,
            "{second:?}: the same glyph outline"
        );
        assert_eq!(
            alone_work,
            prepaid + 32 * alone_outline,
            "{second:?} alone: no probe"
        );
        assert_eq!(
            after_work,
            prepaid + 32 * (1 + after_outline),
            "{second:?} after {first:?}: one probe for its declaring chunk"
        );
    }
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
    let source_and_json_normalization_and_plan = 72 + 11 * 8 + 5 * 8 + 8;
    let format_work = match format {
        OutputFormat::Svg | OutputFormat::Pdf => 0,
        OutputFormat::Png => {
            let rgba_bytes = 4 * 20 * 20;
            let solid_background_samples = 4 * 20 * 20;
            solid_background_samples + rgba_bytes + rgba_bytes
        }
        _ => unreachable!("all revision-1 formats are covered"),
    };
    source_and_json_normalization_and_plan + format_work + output_bytes
}

fn logical_aligned_bytes(bytes: u64) -> u64 {
    bytes.div_ceil(64) * 64
}

/// One Excalifont text element per `(id, text)`, laid side by side.
fn excalifont_scene(elements: &[(&str, &str)]) -> SceneDocument {
    let elements = elements
        .iter()
        .enumerate()
        .map(|(index, (id, text))| {
            format!(
                r#"{{"id":{},"type":"text","x":{},"y":0,"width":160,"height":40,"fontSize":20,"fontFamily":5,"text":{},"updated":1}}"#,
                serde_json::to_string(id).expect("id serializes"),
                index * 200,
                serde_json::to_string(text).expect("text serializes")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"{{"type":"excalidraw","version":2,"elements":[{elements}],"appState":{{}},"files":{{}}}}"#
    );
    SceneDocument::parse(source.as_bytes()).expect("text scene is admitted")
}

/// One text element's `(glyph-outline commands, total work units)`, read from
/// its two local crossings: a one-segment geometry ceiling reports the
/// element's outline-command count, and a work ceiling equal to the work the
/// element prepays before shaping (its scalars, prospective glyphs, and line
/// steps) passes preflight at equality and reports the complete work at the
/// layout crossing. Both crossings omit the element with the exact attempted
/// value in the diagnostic, so nothing here reaches inside the ledger.
fn text_element_work_vector(
    scene: &SceneDocument,
    element_id: &str,
    prepaid_work_units: u64,
) -> (u64, u64) {
    let outline_commands = attempted_at_local_crossing(
        scene,
        element_id,
        ResourceLimits::builder()
            .max_geometry_segments_per_element(1)
            .build()
            .expect("one-segment geometry ceiling is valid"),
        "max_geometry_segments_per_element",
    );
    let work_units = attempted_at_local_crossing(
        scene,
        element_id,
        ResourceLimits::builder()
            .max_element_work_units(prepaid_work_units)
            .build()
            .expect("prepaid work ceiling is valid"),
        "max_element_work_units",
    );
    (outline_commands, work_units)
}

/// The exact attempted value `element_id`'s local crossing of `field` reports
/// when the SVG target renders under `limits`; the element is omitted, so the
/// render succeeds and the value is read from its diagnostic.
fn attempted_at_local_crossing(
    scene: &SceneDocument,
    element_id: &str,
    limits: ResourceLimits,
    field: &str,
) -> u64 {
    let outcome = Renderer::new()
        .render(scene, &request_with_limits(OutputFormat::Svg, limits))
        .expect("a locally omitted text element leaves a meaningful artifact");
    let attempted = outcome
        .report()
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.element_id() == Some(element_id))
        .find_map(|diagnostic| match diagnostic.details() {
            Some(DiagnosticDetails::ResourceLimit {
                field: crossed,
                measurement:
                    ResourceMeasurement::Attempted {
                        value: ResourceValue::Integer(value),
                    },
                ..
            }) if *crossed == field => Some(*value),
            _ => None,
        });
    attempted.unwrap_or_else(|| panic!("{element_id} crosses {field}"))
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
