use excalidraw_render::{
    ColorMode, DiagnosticDetails, ErrorCode, ErrorFamily, FrameOptions, MetadataOptions,
    OutputFormat, PdfOptions, PdfRasterFallback, RenderRequest, ResourceLimits, Sizing,
    SupplementalAssets,
};

#[test]
fn one_format_requests_have_the_revision_1_defaults() {
    for (format, serialized_format, media_type) in [
        (OutputFormat::Svg, "\"svg\"", "image/svg+xml"),
        (OutputFormat::Png, "\"png\"", "image/png"),
        (OutputFormat::Pdf, "\"pdf\"", "application/pdf"),
    ] {
        let request = RenderRequest::builder(format)
            .build()
            .expect("revision-1 defaults are valid");

        assert_eq!(request.format(), format);
        assert_eq!(request.format().media_type(), media_type);
        assert!(request.include_background());
        assert_eq!(request.padding(), 10.0);
        assert_eq!(request.sizing(), Sizing::Scale(1.0));
        assert_eq!(request.color_mode(), ColorMode::Light);
        assert_eq!(request.frame_options(), &FrameOptions::default());
        assert_eq!(request.metadata_options(), &MetadataOptions::default());
        assert!(request.supplemental_assets().is_empty());
        assert_eq!(request.resource_limits(), &Default::default());
        assert_eq!(
            request
                .pdf_options()
                .map(|options| options.raster_fallback()),
            (format == OutputFormat::Pdf).then_some(PdfRasterFallback::AllowIsolated)
        );
        assert_eq!(
            serde_json::to_string(&format).expect("serialize output format"),
            serialized_format
        );
    }
}

#[test]
fn frame_and_metadata_options_start_from_documented_defaults() {
    let frames = FrameOptions::default();
    assert!(frames.enabled() && frames.clip() && frames.show_name() && frames.outline());
    let metadata = MetadataOptions::default();
    assert!(!metadata.embedded_scene() && !metadata.provenance() && !metadata.diagnostic_summary());

    let frames = FrameOptions::default()
        .with_enabled(false)
        .with_clip(false)
        .with_show_name(true)
        .with_outline(false);
    assert_eq!(
        (
            frames.enabled(),
            frames.clip(),
            frames.show_name(),
            frames.outline()
        ),
        (false, false, true, false)
    );
    for (options, expected) in [
        (
            MetadataOptions::default().with_embedded_scene(true),
            (true, false, false),
        ),
        (
            MetadataOptions::default().with_provenance(true),
            (false, true, false),
        ),
        (
            MetadataOptions::default().with_diagnostic_summary(true),
            (false, false, true),
        ),
    ] {
        assert_eq!(
            (
                options.embedded_scene(),
                options.provenance(),
                options.diagnostic_summary()
            ),
            expected
        );
    }
    // A setter replaces its own control, so the last call wins.
    assert_eq!(
        FrameOptions::default().with_clip(false).with_clip(true),
        FrameOptions::default()
    );
}

#[test]
fn approved_request_controls_are_retained() {
    let frame_options = FrameOptions::default()
        .with_enabled(false)
        .with_clip(false)
        .with_outline(false);
    let metadata_options = MetadataOptions::default()
        .with_embedded_scene(true)
        .with_provenance(true)
        .with_diagnostic_summary(true);
    let request = RenderRequest::builder(OutputFormat::Png)
        .include_background(false)
        .padding(2.5)
        .sizing(Sizing::MaxDimension(640.0))
        .color_mode(ColorMode::Dark)
        .frame_options(frame_options.clone())
        .metadata_options(metadata_options.clone())
        .build()
        .expect("approved request controls are valid");

    assert!(!request.include_background());
    assert_eq!(request.padding(), 2.5);
    assert_eq!(request.sizing(), Sizing::MaxDimension(640.0));
    assert_eq!(request.color_mode(), ColorMode::Dark);
    assert_eq!(request.frame_options(), &frame_options);
    assert_eq!(request.metadata_options(), &metadata_options);
    assert!(!request.frame_options().enabled());
    assert!(!request.frame_options().clip());
    assert!(request.frame_options().show_name());
    assert!(!request.frame_options().outline());
    assert!(request.metadata_options().embedded_scene());
    assert!(request.metadata_options().provenance());
    assert!(request.metadata_options().diagnostic_summary());
}

#[test]
fn numeric_request_values_are_validated_before_rendering() {
    let cases = [
        (
            RenderRequest::builder(OutputFormat::Svg).padding(-1.0),
            ErrorCode::REQUEST_INVALID_PADDING,
            "padding",
        ),
        (
            RenderRequest::builder(OutputFormat::Svg).padding(f64::NAN),
            ErrorCode::REQUEST_INVALID_PADDING,
            "padding",
        ),
        (
            RenderRequest::builder(OutputFormat::Png).sizing(Sizing::Scale(0.0)),
            ErrorCode::REQUEST_INVALID_SCALE,
            "scale",
        ),
        (
            RenderRequest::builder(OutputFormat::Png).sizing(Sizing::Scale(f64::INFINITY)),
            ErrorCode::REQUEST_INVALID_SCALE,
            "scale",
        ),
        (
            RenderRequest::builder(OutputFormat::Pdf).sizing(Sizing::MaxDimension(-0.1)),
            ErrorCode::REQUEST_INVALID_MAX_DIMENSION,
            "max_dimension",
        ),
        (
            RenderRequest::builder(OutputFormat::Pdf).sizing(Sizing::MaxDimension(f64::NAN)),
            ErrorCode::REQUEST_INVALID_MAX_DIMENSION,
            "max_dimension",
        ),
    ];

    for (builder, expected_code, expected_field) in cases {
        let error = builder.build().expect_err("invalid value is rejected");
        assert_eq!(error.code(), expected_code);
        assert_eq!(error.field(), expected_field);
        assert_eq!(error.asset_id(), None);
        assert_eq!(error.option(), None);
        assert_eq!(error.descriptor().family(), ErrorFamily::Request);
        assert_eq!(error.descriptor().category(), None);
        assert_eq!(error.descriptor().json_pointer(), None);
        assert!(matches!(
            error.descriptor().details(),
            Some(DiagnosticDetails::RequestValue {
                field,
                asset_id: None,
                option: None,
                ..
            }) if *field == expected_field
        ));
    }

    let error = RenderRequest::builder(OutputFormat::Svg)
        .padding(f64::NEG_INFINITY)
        .build()
        .expect_err("non-finite padding is rejected");
    assert_eq!(
        serde_json::to_string(&error.descriptor()).expect("serialize request descriptor"),
        concat!(
            "{\"family\":\"request\",",
            "\"category\":null,",
            "\"code\":\"request.invalid-padding\",",
            "\"message\":\"padding must be finite and nonnegative, got -inf\",",
            "\"format\":null,",
            "\"element_id\":null,",
            "\"json_pointer\":null,",
            "\"details\":{\"kind\":\"request_value\",",
            "\"field\":\"padding\",",
            "\"asset_id\":null,",
            "\"option\":null}}"
        )
    );
}

#[test]
fn pdf_options_are_rejected_for_other_formats() {
    for format in [OutputFormat::Svg, OutputFormat::Png] {
        let error = RenderRequest::builder(format)
            .pdf_options(PdfOptions::default().with_raster_fallback(PdfRasterFallback::Forbid))
            .build()
            .expect_err("PDF options cannot configure another format");

        assert_eq!(error.code(), ErrorCode::REQUEST_INCOMPATIBLE_FORMAT_OPTIONS);
        assert_eq!(error.field(), "pdf_options");
        assert_eq!(error.option(), Some("raster_fallback"));
    }

    let request = RenderRequest::builder(OutputFormat::Pdf)
        .pdf_options(PdfOptions::default().with_raster_fallback(PdfRasterFallback::Forbid))
        .build()
        .expect("PDF options configure a PDF request");
    assert_eq!(
        request.pdf_options().map(PdfOptions::raster_fallback),
        Some(PdfRasterFallback::Forbid)
    );
}

#[test]
fn supplemental_assets_own_bytes_and_iterate_by_file_id() {
    let source_bytes = vec![1, 2, 3];
    let assets = SupplementalAssets::builder()
        .asset("z-last", source_bytes.clone())
        .asset_with_media_type("a-first", vec![4, 5], "image/png")
        .build()
        .expect("supplemental assets are valid");
    drop(source_bytes);

    assert_eq!(assets.len(), 2);
    assert_eq!(
        assets.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        ["a-first", "z-last"]
    );
    assert_eq!(
        assets.get("z-last").map(|asset| asset.bytes()),
        Some(&[1, 2, 3][..])
    );
    assert_eq!(
        assets
            .get("a-first")
            .and_then(|asset| asset.advisory_media_type()),
        Some("image/png")
    );

    let request = RenderRequest::builder(OutputFormat::Svg)
        .supplemental_assets(assets)
        .build()
        .expect("owned supplemental assets form a valid request");
    assert_eq!(request.supplemental_assets().len(), 2);
}

#[test]
fn supplemental_asset_identifiers_and_media_types_are_validated() {
    let empty_id = SupplementalAssets::builder()
        .asset("", vec![1])
        .build()
        .expect_err("empty file IDs are rejected");
    assert_eq!(empty_id.code(), ErrorCode::REQUEST_EMPTY_ASSET_ID);
    assert_eq!(empty_id.field(), "supplemental_assets");
    assert_eq!(empty_id.asset_id(), Some(""));
    assert_eq!(empty_id.option(), None);

    let duplicate = SupplementalAssets::builder()
        .asset("image-1", vec![1])
        .asset("image-1", vec![2])
        .build()
        .expect_err("duplicate file IDs are rejected");
    assert_eq!(duplicate.code(), ErrorCode::REQUEST_DUPLICATE_ASSET_ID);
    assert_eq!(duplicate.asset_id(), Some("image-1"));
    assert_eq!(duplicate.option(), None);

    for invalid_media_type in ["image", "image/ png", "image/png;"] {
        let error = SupplementalAssets::builder()
            .asset_with_media_type("image-1", vec![1], invalid_media_type)
            .build()
            .expect_err("invalid advisory media type is rejected");
        assert_eq!(error.code(), ErrorCode::REQUEST_INVALID_ASSET_MEDIA_TYPE);
        assert_eq!(error.field(), "supplemental_assets");
        assert_eq!(error.asset_id(), Some("image-1"));
        assert_eq!(error.option(), Some("media_type"));
    }
}

#[test]
fn semantic_resource_limits_expose_exact_defaults_and_absolute_maxima() {
    let defaults = ResourceLimits::default();
    let maxima = ResourceLimits::absolute_maxima();

    assert_eq!(defaults.max_scene_bytes(), 32 * 1_048_576);
    assert_eq!(maxima.max_scene_bytes(), 64 * 1_048_576);
    assert_eq!(defaults.max_elements(), 16_384);
    assert_eq!(maxima.max_elements(), 65_536);
    assert_eq!(defaults.max_asset_entries(), 1_024);
    assert_eq!(maxima.max_asset_entries(), 4_096);
    assert_eq!(defaults.max_asset_bytes(), 32 * 1_048_576);
    assert_eq!(maxima.max_asset_bytes(), 64 * 1_048_576);
    assert_eq!(defaults.max_total_asset_bytes(), 128 * 1_048_576);
    assert_eq!(maxima.max_total_asset_bytes(), 256 * 1_048_576);
    assert_eq!(defaults.max_relationship_depth(), 64);
    assert_eq!(maxima.max_relationship_depth(), 128);
    assert_eq!(defaults.max_nested_asset_depth(), 4);
    assert_eq!(maxima.max_nested_asset_depth(), 8);
    assert_eq!(defaults.max_nested_svg_nodes(), 65_536);
    assert_eq!(maxima.max_nested_svg_nodes(), 262_144);
    assert_eq!(defaults.max_geometry_segments_per_element(), 262_144);
    assert_eq!(maxima.max_geometry_segments_per_element(), 1_048_576);
    assert_eq!(defaults.max_total_geometry_segments(), 4_194_304);
    assert_eq!(maxima.max_total_geometry_segments(), 16_777_216);
    assert_eq!(defaults.max_text_scalars_per_element(), 262_144);
    assert_eq!(maxima.max_text_scalars_per_element(), 1_048_576);
    assert_eq!(defaults.max_total_text_scalars(), 4_194_304);
    assert_eq!(maxima.max_total_text_scalars(), 16_777_216);
    assert_eq!(defaults.max_raster_dimension(), 16_384);
    assert_eq!(maxima.max_raster_dimension(), 32_768);
    assert_eq!(defaults.max_decoded_pixels_per_asset(), 16_777_216);
    assert_eq!(maxima.max_decoded_pixels_per_asset(), 67_108_864);
    assert_eq!(defaults.max_total_decoded_pixels(), 33_554_432);
    assert_eq!(maxima.max_total_decoded_pixels(), 134_217_728);
    assert_eq!(defaults.max_raster_surface_pixels(), 33_554_432);
    assert_eq!(maxima.max_raster_surface_pixels(), 134_217_728);
    assert_eq!(defaults.max_total_rasterized_pixels(), 67_108_864);
    assert_eq!(maxima.max_total_rasterized_pixels(), 268_435_456);
    assert_eq!(defaults.max_svg_dimension(), 1_000_000.0);
    assert_eq!(maxima.max_svg_dimension(), 10_000_000.0);
    assert_eq!(defaults.max_pdf_page_points(), 14_400.0);
    assert_eq!(maxima.max_pdf_page_points(), 14_400.0);
    assert_eq!(defaults.max_coordinate_magnitude(), 1e9);
    assert_eq!(maxima.max_coordinate_magnitude(), 1e12);
    assert_eq!(defaults.max_diagnostics(), 65_536);
    assert_eq!(maxima.max_diagnostics(), 262_144);
    assert_eq!(defaults.max_working_bytes(), 256 * 1_048_576);
    assert_eq!(maxima.max_working_bytes(), 512 * 1_048_576);
    assert_eq!(defaults.max_output_bytes(), 128 * 1_048_576);
    assert_eq!(maxima.max_output_bytes(), 512 * 1_048_576);
    assert_eq!(defaults.max_element_work_units(), 67_108_864);
    assert_eq!(maxima.max_element_work_units(), 268_435_456);
    assert_eq!(defaults.max_asset_work_units(), 134_217_728);
    assert_eq!(maxima.max_asset_work_units(), 536_870_912);
    assert_eq!(defaults.max_total_work_units(), 4_294_967_296);
    assert_eq!(maxima.max_total_work_units(), 17_179_869_184);
}

#[test]
fn semantic_resource_limits_are_constructed_only_through_validation() {
    let defaults = ResourceLimits::default();
    let rebuilt = ResourceLimits::builder()
        .max_scene_bytes(defaults.max_scene_bytes())
        .max_elements(defaults.max_elements())
        .max_asset_entries(defaults.max_asset_entries())
        .max_asset_bytes(defaults.max_asset_bytes())
        .max_total_asset_bytes(defaults.max_total_asset_bytes())
        .max_relationship_depth(defaults.max_relationship_depth())
        .max_nested_asset_depth(defaults.max_nested_asset_depth())
        .max_nested_svg_nodes(defaults.max_nested_svg_nodes())
        .max_geometry_segments_per_element(defaults.max_geometry_segments_per_element())
        .max_total_geometry_segments(defaults.max_total_geometry_segments())
        .max_text_scalars_per_element(defaults.max_text_scalars_per_element())
        .max_total_text_scalars(defaults.max_total_text_scalars())
        .max_raster_dimension(defaults.max_raster_dimension())
        .max_decoded_pixels_per_asset(defaults.max_decoded_pixels_per_asset())
        .max_total_decoded_pixels(defaults.max_total_decoded_pixels())
        .max_raster_surface_pixels(defaults.max_raster_surface_pixels())
        .max_total_rasterized_pixels(defaults.max_total_rasterized_pixels())
        .max_svg_dimension(defaults.max_svg_dimension())
        .max_pdf_page_points(defaults.max_pdf_page_points())
        .max_coordinate_magnitude(defaults.max_coordinate_magnitude())
        .max_diagnostics(defaults.max_diagnostics())
        .max_working_bytes(defaults.max_working_bytes())
        .max_output_bytes(defaults.max_output_bytes())
        .max_element_work_units(defaults.max_element_work_units())
        .max_asset_work_units(defaults.max_asset_work_units())
        .max_total_work_units(defaults.max_total_work_units())
        .build()
        .expect("the default profile validates");
    assert_eq!(rebuilt, defaults);

    let request = RenderRequest::builder(OutputFormat::Png)
        .resource_limits(rebuilt)
        .build()
        .expect("validated limits form a valid request");
    assert_eq!(request.resource_limits(), &defaults);
}

#[test]
fn invalid_and_inconsistent_resource_limits_return_the_exact_request_failure() {
    let invalid = ResourceLimits::builder()
        .max_scene_bytes(0)
        .build()
        .expect_err("zero limits are rejected");
    assert_eq!(invalid.code(), ErrorCode::REQUEST_INVALID_RESOURCE_LIMIT);
    assert_eq!(invalid.field(), "max_scene_bytes");
    assert_eq!(invalid.asset_id(), None);
    assert_eq!(invalid.option(), None);
    assert_eq!(
        serde_json::to_string(&invalid.descriptor()).expect("serialize resource-limit error"),
        concat!(
            "{\"family\":\"request\",",
            "\"category\":null,",
            "\"code\":\"request.invalid-resource-limit\",",
            "\"message\":\"resource limit max_scene_bytes must be between 1 and 67108864, got 0\",",
            "\"format\":null,",
            "\"element_id\":null,",
            "\"json_pointer\":null,",
            "\"details\":{\"kind\":\"request_value\",",
            "\"field\":\"max_scene_bytes\",",
            "\"asset_id\":null,",
            "\"option\":null}}"
        )
    );

    for (builder, expected_field, expected_message) in [
        (
            ResourceLimits::builder().max_coordinate_magnitude(f64::INFINITY),
            "max_coordinate_magnitude",
            "resource limit max_coordinate_magnitude must be finite, positive and at most 1000000000000, got inf",
        ),
        (
            ResourceLimits::builder().max_pdf_page_points(14_401.0),
            "max_pdf_page_points",
            "resource limit max_pdf_page_points must be finite, positive and at most 14400, got 14401",
        ),
        (
            ResourceLimits::builder()
                .max_asset_bytes(20)
                .max_total_asset_bytes(10),
            "max_asset_bytes",
            "resource limit max_asset_bytes (20) must not exceed max_total_asset_bytes (10)",
        ),
        (
            ResourceLimits::builder()
                .max_geometry_segments_per_element(20)
                .max_total_geometry_segments(10),
            "max_geometry_segments_per_element",
            "resource limit max_geometry_segments_per_element (20) must not exceed max_total_geometry_segments (10)",
        ),
        (
            ResourceLimits::builder()
                .max_text_scalars_per_element(20)
                .max_total_text_scalars(10),
            "max_text_scalars_per_element",
            "resource limit max_text_scalars_per_element (20) must not exceed max_total_text_scalars (10)",
        ),
        (
            ResourceLimits::builder()
                .max_decoded_pixels_per_asset(20)
                .max_total_decoded_pixels(10),
            "max_decoded_pixels_per_asset",
            "resource limit max_decoded_pixels_per_asset (20) must not exceed max_total_decoded_pixels (10)",
        ),
        (
            ResourceLimits::builder()
                .max_raster_surface_pixels(20)
                .max_total_rasterized_pixels(10),
            "max_raster_surface_pixels",
            "resource limit max_raster_surface_pixels (20) must not exceed max_total_rasterized_pixels (10)",
        ),
        (
            ResourceLimits::builder()
                .max_element_work_units(20)
                .max_total_work_units(10),
            "max_element_work_units",
            "resource limit max_element_work_units (20) must not exceed max_total_work_units (10)",
        ),
        (
            ResourceLimits::builder()
                .max_element_work_units(10)
                .max_asset_work_units(20)
                .max_total_work_units(10),
            "max_asset_work_units",
            "resource limit max_asset_work_units (20) must not exceed max_total_work_units (10)",
        ),
    ] {
        let error = builder
            .build()
            .expect_err("inconsistent limits are rejected");
        assert_eq!(error.code(), ErrorCode::REQUEST_INVALID_RESOURCE_LIMIT);
        assert_eq!(error.field(), expected_field);
        assert_eq!(error.to_string(), expected_message);
        assert_eq!(error.descriptor().message(), expected_message);
    }
}
