mod pdf;
mod png;
mod rough;
mod svg;

use crate::{
    resource_accounting::{
        ResourceContext, ResourceCrossing, ResourceDelta, ResourceMeter, ResourcePhase,
        ResourceRecordKind,
    },
    scene::{OriginalJsonEntryKind, OriginalJsonVisitError},
    Artifact, CompatibilityDiagnostic, CompatibilityReport, DiagnosticCode, DiagnosticEffect,
    DiagnosticSeverity, ErrorCode, ErrorDetails, FiniteValue, Measurement, OutputFormat, Recovery,
    ReferenceProfileIdentity, RenderError, RenderErrorCategory, RenderOutcome, RenderRequest,
    ResourceValue, SceneDocument, Sizing, ValueKind,
};

use self::{pdf::PdfPlan, png::PngPlan, svg::SvgPlan};

const RECTANGLE_NORMALIZED_FIELD_COUNT: u64 = 15;
const RECTANGLE_GEOMETRY_SEGMENTS: u64 = 16;
const RECTANGLE_GEOMETRY_WORK_UNITS: u64 = RECTANGLE_GEOMETRY_SEGMENTS * 32;
const RECTANGLE_BASE_ELEMENT_WORK_UNITS: u64 =
    RECTANGLE_NORMALIZED_FIELD_COUNT * 8 + RECTANGLE_GEOMETRY_WORK_UNITS + 8;

pub(super) enum FormatEncodeError {
    Encoding(String),
    MemoryUnavailable {
        subject_kind: &'static str,
        subject: &'static str,
    },
}

pub(crate) fn render(
    profile: &'static ReferenceProfileIdentity,
    scene: &SceneDocument,
    request: &RenderRequest,
) -> Result<RenderOutcome, RenderError> {
    let mut session = RenderSession::new(profile, request);
    session.render(scene, request)
}

#[cfg(test)]
fn render_with_trace(
    profile: &'static ReferenceProfileIdentity,
    scene: &SceneDocument,
    request: &RenderRequest,
) -> (Result<RenderOutcome, RenderError>, Vec<u8>) {
    let mut session = RenderSession::new_with_trace(profile, scene, request);
    let result = session.render(scene, request);
    (result, session.meter.trace_bytes())
}

fn format_finite_number(value: f64, subject: &str) -> Result<String, String> {
    let value = FiniteValue::new(value).ok_or_else(|| format!("{subject} must be finite"))?;
    serde_json::to_string(&value).map_err(|_| format!("failed to serialize {subject}"))
}

struct RenderSession {
    profile: &'static ReferenceProfileIdentity,
    report: CompatibilityReport,
    meter: ResourceMeter,
    staged_output: Option<Vec<u8>>,
}

impl RenderSession {
    fn new(profile: &'static ReferenceProfileIdentity, request: &RenderRequest) -> Self {
        Self {
            profile,
            report: CompatibilityReport::empty(),
            meter: ResourceMeter::new(request.resource_limits()),
            staged_output: None,
        }
    }

    #[cfg(test)]
    fn new_with_trace(
        profile: &'static ReferenceProfileIdentity,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Self {
        Self {
            profile,
            report: CompatibilityReport::empty(),
            meter: ResourceMeter::with_trace(profile, scene, request),
            staged_output: None,
        }
    }

    fn render(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Result<RenderOutcome, RenderError> {
        self.transact(
            request,
            ResourceContext::new(ResourcePhase::Admission, "request_session", "")
                .with_record(ResourceRecordKind::RequestSessionMeter),
            ResourceDelta {
                working_bytes: 4_096,
                ..ResourceDelta::default()
            },
        )?;
        self.transact(
            request,
            ResourceContext::new(ResourcePhase::Admission, "scene_source_octets", ""),
            ResourceDelta {
                scene_bytes: scene.source_bytes(),
                total_work_units: scene.source_bytes(),
                ..ResourceDelta::default()
            },
        )?;
        let traversal = scene.visit_original_json_entries(|entry| {
            let event = match entry.kind {
                OriginalJsonEntryKind::Node => "json_node",
                OriginalJsonEntryKind::Member => "json_member",
            };
            self.transact(
                request,
                ResourceContext::new(
                    ResourcePhase::JsonTraversalAndNormalization,
                    event,
                    entry.json_pointer,
                ),
                ResourceDelta {
                    elements: u64::from(entry.is_element_occurrence),
                    total_work_units: 8,
                    ..ResourceDelta::default()
                },
            )
        });
        match traversal {
            Ok(()) => {}
            Err(OriginalJsonVisitError::Visitor(error)) => return Err(error),
            Err(OriginalJsonVisitError::MemoryUnavailable { json_pointer }) => {
                return Err(self.memory_failure_at(
                    request,
                    "semantic_traversal",
                    "scene_json_pointer",
                    None,
                    json_pointer,
                ));
            }
        }

        let semantic_working_bytes;
        let resolved = if scene.has_no_element_occurrences() {
            semantic_working_bytes = 0;
            ResolvedScene::empty(request)
        } else {
            if request.format() == OutputFormat::Pdf {
                return Err(self.internal_failure(
                    request.format(),
                    "nonempty rendering is not implemented for the selected format",
                    "/elements",
                ));
            }
            let source = scene.single_rectangle().ok_or_else(|| {
                self.internal_failure(
                    request.format(),
                    "nonempty scene is outside the implemented rectangle slice",
                    "/elements",
                )
            })?;
            let normalized_working_bytes = 256
                + 10 * 64
                + owned_field_bytes(source.id)
                + owned_field_bytes(source.stroke_color)
                + owned_field_bytes(source.background_color)
                + owned_field_bytes(source.fill_style)
                + owned_field_bytes(source.stroke_style)
                + source.view_background_color.map_or(0, owned_field_bytes);
            let geometry_working_bytes = 128 + RECTANGLE_GEOMETRY_SEGMENTS * 64;
            self.transact(
                request,
                ResourceContext::new(
                    ResourcePhase::JsonTraversalAndNormalization,
                    "normalized_rectangle",
                    "/elements/0",
                )
                .with_element_id(Some(source.id))
                .with_local_owner(Some(source.id)),
                ResourceDelta {
                    working_bytes: normalized_working_bytes,
                    total_work_units: RECTANGLE_NORMALIZED_FIELD_COUNT * 8,
                    ..ResourceDelta::default()
                },
            )?;
            let geometry_delta = ResourceDelta {
                geometry_segments: RECTANGLE_GEOMETRY_SEGMENTS,
                working_bytes: geometry_working_bytes,
                total_work_units: RECTANGLE_GEOMETRY_WORK_UNITS,
                ..ResourceDelta::default()
            };
            let geometry_context = ResourceContext::new(
                ResourcePhase::SharedSemanticResolution,
                "rough_rectangle_geometry",
                "/elements/0",
            )
            .with_element_id(Some(source.id))
            .with_local_owner(Some(source.id));
            if self.meter.would_cross_fatal(geometry_delta) {
                return match self.transact(request, geometry_context, geometry_delta) {
                    Err(error) => Err(error),
                    Ok(()) => Err(self.internal_failure(
                        request.format(),
                        "fatal resource preflight committed unexpectedly",
                        "/elements/0",
                    )),
                };
            }
            let local_crossing = rectangle_local_crossing(
                source,
                request,
                RECTANGLE_GEOMETRY_SEGMENTS,
                RECTANGLE_BASE_ELEMENT_WORK_UNITS,
            );
            let include_outline = local_crossing.is_none();
            if let Some(crossing) = local_crossing {
                let diagnostic =
                    crossing
                        .diagnostic(request.format(), source.id)
                        .map_err(|_| {
                            self.memory_failure(request, "compatibility_report", "local_recovery")
                        })?;
                self.retain_diagnostic(
                    request,
                    ResourcePhase::SharedSemanticResolution,
                    "rectangle_local_resource_recovery",
                    diagnostic,
                )?;
                semantic_working_bytes = normalized_working_bytes;
            } else {
                self.transact(request, geometry_context, geometry_delta)?;
                semantic_working_bytes =
                    normalized_working_bytes.saturating_add(geometry_working_bytes);
            }
            match ResolvedScene::rectangle(source, request, include_outline) {
                Ok(Some(resolved)) => resolved,
                Ok(None) => {
                    return Err(self.internal_failure(
                        request.format(),
                        "rectangle semantics are outside the implemented focused case",
                        "/elements/0",
                    ));
                }
                Err(_) => {
                    return Err(self.memory_failure(request, "semantic_geometry", "rectangle"));
                }
            }
        };
        let bytes = self.encode(resolved, request, semantic_working_bytes)?;

        Ok(RenderOutcome {
            artifact: Artifact {
                format: request.format(),
                bytes,
            },
            report: std::mem::take(&mut self.report),
            profile: self.profile,
        })
    }

    fn encode(
        &mut self,
        resolved: ResolvedScene,
        request: &RenderRequest,
        semantic_working_bytes: u64,
    ) -> Result<Vec<u8>, RenderError> {
        if request.format() == OutputFormat::Svg {
            self.validate_svg_dimensions(&resolved, request)?;
        }
        let plan_base = ResourceContext::new(ResourcePhase::FormatPlanning, "format_plan_base", "")
            .with_record(ResourceRecordKind::FormatPlanBase);
        self.acquire_working(request, plan_base, 1_024)?;
        let operation_bytes = if resolved.include_background { 128 } else { 0 };
        let background_operation =
            ResourceContext::new(ResourcePhase::FormatPlanning, "background_operation", "")
                .with_record(ResourceRecordKind::FormatPlanOperation);
        if operation_bytes != 0 {
            self.transact(
                request,
                background_operation,
                ResourceDelta {
                    working_bytes: operation_bytes,
                    total_work_units: 8,
                    ..ResourceDelta::default()
                },
            )?;
        }
        match request.format() {
            OutputFormat::Svg => {
                if let Some((element_id, json_pointer)) = resolved.primary_source() {
                    self.transact(
                        request,
                        ResourceContext::new(
                            ResourcePhase::FormatPlanning,
                            "element_operation",
                            json_pointer,
                        )
                        .with_element_id(Some(element_id))
                        .with_local_owner(Some(element_id))
                        .with_record(ResourceRecordKind::FormatPlanOperation),
                        ResourceDelta {
                            working_bytes: 128,
                            total_work_units: 8,
                            ..ResourceDelta::default()
                        },
                    )?;
                }
                let plan = SvgPlan::new(resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_SVG_FAILED, error)
                })?;
                if semantic_working_bytes != 0 {
                    let mut context = ResourceContext::new(
                        ResourcePhase::FormatPlanning,
                        "resolved_rectangle",
                        "/elements/0",
                    );
                    if let Some((element_id, _)) = plan.primary_source() {
                        context = context
                            .with_element_id(Some(element_id))
                            .with_local_owner(Some(element_id));
                    }
                    self.release_working(request, context, semantic_working_bytes)?;
                }
                let workspace_bytes = 256 * 1_024;
                let workspace = ResourceContext::new(
                    ResourcePhase::EncodingAndOutcomeAssembly,
                    "svg_serializer_workspace",
                    "",
                )
                .with_record(ResourceRecordKind::SvgSerializerWorkspace);
                self.acquire_working(request, workspace, workspace_bytes)?;
                self.measure_and_emit_output(request, ErrorCode::ENCODE_SVG_FAILED, 0, || {
                    plan.encode()
                })?;
                self.release_working(request, workspace, workspace_bytes)?;
                if let Some((element_id, json_pointer)) = plan.primary_source() {
                    self.release_working(
                        request,
                        ResourceContext::new(
                            ResourcePhase::EncodingAndOutcomeAssembly,
                            "element_operation",
                            json_pointer,
                        )
                        .with_element_id(Some(element_id))
                        .with_local_owner(Some(element_id))
                        .with_record(ResourceRecordKind::FormatPlanOperation),
                        128,
                    )?;
                }
            }
            OutputFormat::Png => {
                if let Some((element_id, json_pointer)) = resolved.primary_source() {
                    self.transact(
                        request,
                        ResourceContext::new(
                            ResourcePhase::FormatPlanning,
                            "element_operation",
                            json_pointer,
                        )
                        .with_element_id(Some(element_id))
                        .with_local_owner(Some(element_id))
                        .with_record(ResourceRecordKind::FormatPlanOperation),
                        ResourceDelta {
                            working_bytes: 128,
                            total_work_units: 8,
                            ..ResourceDelta::default()
                        },
                    )?;
                }
                let mut plan = PngPlan::new(resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_PNG_FAILED, error)
                })?;
                if semantic_working_bytes != 0 {
                    let mut context = ResourceContext::new(
                        ResourcePhase::FormatPlanning,
                        "resolved_rectangle",
                        "/elements/0",
                    );
                    if let Some((element_id, _)) = plan.primary_source() {
                        context = context
                            .with_element_id(Some(element_id))
                            .with_local_owner(Some(element_id));
                    }
                    self.release_working(request, context, semantic_working_bytes)?;
                }
                let pixels = u64::from(plan.width()).saturating_mul(u64::from(plan.height()));
                let rgba_bytes = pixels.saturating_mul(4);
                let surface_bytes = 256_u64.saturating_add(rgba_bytes);
                let surface = ResourceContext::new(
                    ResourcePhase::ProjectionAndFallback,
                    "main_raster_surface",
                    "",
                )
                .with_record(ResourceRecordKind::RasterColorSurface);
                self.acquire_working(request, surface, surface_bytes)?;

                let raster_work = if plan.includes_background() {
                    pixels.saturating_mul(4)
                } else {
                    pixels
                };
                let raster_event = if plan.includes_background() {
                    "solid_background_samples"
                } else {
                    "clear_samples"
                };
                self.charge_work(
                    request,
                    ResourceContext::new(ResourcePhase::ProjectionAndFallback, raster_event, ""),
                    raster_work,
                )?;
                if let Some((element_id, json_pointer)) = plan.primary_source() {
                    let vector_work = plan.vector_coverage_area().saturating_mul(16);
                    let context = ResourceContext::new(
                        ResourcePhase::ProjectionAndFallback,
                        "vector_coverage_samples",
                        json_pointer,
                    )
                    .with_element_id(Some(element_id))
                    .with_local_owner(Some(element_id));
                    let delta = ResourceDelta {
                        total_work_units: vector_work,
                        ..ResourceDelta::default()
                    };
                    if self.meter.would_cross_fatal(delta) {
                        self.charge_work(request, context, vector_work)?;
                    }
                    let element_work =
                        RECTANGLE_BASE_ELEMENT_WORK_UNITS.saturating_add(vector_work);
                    let element_work_limit = request.resource_limits().max_element_work_units();
                    if element_work > element_work_limit {
                        let diagnostic = RectangleLocalCrossing {
                            code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
                            field: "max_element_work_units",
                            configured: ResourceValue::Integer(element_work_limit),
                            value_kind: ValueKind::WorkUnits,
                            attempted: ResourceValue::Integer(element_work),
                            json_pointer: "/elements/0",
                            message:
                                "rectangle omitted because its semantic work exceeds its limit",
                        }
                        .diagnostic(request.format(), element_id)
                        .map_err(|_| {
                            self.memory_failure(request, "compatibility_report", "local_recovery")
                        })?;
                        self.retain_diagnostic(
                            request,
                            ResourcePhase::ProjectionAndFallback,
                            "rectangle_local_resource_recovery",
                            diagnostic,
                        )?;
                        plan.omit_elements();
                    } else {
                        self.charge_work(request, context, vector_work)?;
                    }
                }

                let workspace_bytes = 2_u64
                    .saturating_mul(1_048_576)
                    .saturating_add(16_u64.saturating_mul(u64::from(plan.width())));
                let workspace = ResourceContext::new(
                    ResourcePhase::EncodingAndOutcomeAssembly,
                    "png_encoder_workspace",
                    "",
                )
                .with_record(ResourceRecordKind::PngEncoderWorkspace);
                self.acquire_working(request, workspace, workspace_bytes)?;
                self.charge_work(
                    request,
                    ResourceContext::new(
                        ResourcePhase::EncodingAndOutcomeAssembly,
                        "codec_measurement_input",
                        "",
                    ),
                    rgba_bytes,
                )?;
                self.measure_and_emit_output(
                    request,
                    ErrorCode::ENCODE_PNG_FAILED,
                    rgba_bytes,
                    || plan.encode(),
                )?;
                self.release_working(request, workspace, workspace_bytes)?;
                self.release_working(request, surface, surface_bytes)?;
                if let Some((element_id, json_pointer)) = plan.primary_source() {
                    self.release_working(
                        request,
                        ResourceContext::new(
                            ResourcePhase::EncodingAndOutcomeAssembly,
                            "element_operation",
                            json_pointer,
                        )
                        .with_element_id(Some(element_id))
                        .with_local_owner(Some(element_id))
                        .with_record(ResourceRecordKind::FormatPlanOperation),
                        128,
                    )?;
                }
            }
            OutputFormat::Pdf => {
                let workspace_bytes = 1_048_576;
                let plan = PdfPlan::new(&resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_PDF_FAILED, error)
                })?;
                let workspace = ResourceContext::new(
                    ResourcePhase::EncodingAndOutcomeAssembly,
                    "pdf_stream_workspace",
                    "",
                )
                .with_record(ResourceRecordKind::PdfStreamWorkspace);
                self.acquire_working(request, workspace, workspace_bytes)?;
                self.measure_and_emit_output(request, ErrorCode::ENCODE_PDF_FAILED, 0, || {
                    plan.encode()
                })?;
                self.release_working(request, workspace, workspace_bytes)?;
            }
        }

        if operation_bytes != 0 {
            self.release_working(request, background_operation, operation_bytes)?;
        }
        self.release_working(request, plan_base, 1_024)?;

        self.staged_output.take().ok_or_else(|| {
            self.internal_failure(request.format(), "accepted output bytes are missing", "")
        })
    }

    fn acquire_working(
        &mut self,
        request: &RenderRequest,
        context: ResourceContext<'_>,
        working_bytes: u64,
    ) -> Result<(), RenderError> {
        self.transact(
            request,
            context,
            ResourceDelta {
                working_bytes,
                ..ResourceDelta::default()
            },
        )
    }

    fn charge_work(
        &mut self,
        request: &RenderRequest,
        context: ResourceContext<'_>,
        total_work_units: u64,
    ) -> Result<(), RenderError> {
        self.transact(
            request,
            context,
            ResourceDelta {
                total_work_units,
                ..ResourceDelta::default()
            },
        )
    }

    #[allow(dead_code)]
    fn retain_diagnostic(
        &mut self,
        request: &RenderRequest,
        phase: ResourcePhase,
        event: &'static str,
        diagnostic: CompatibilityDiagnostic,
    ) -> Result<(), RenderError> {
        let pointer_bytes = u64::try_from(diagnostic.json_pointer.len()).unwrap_or(u64::MAX);
        let element_id_bytes = diagnostic.element_id.as_deref().map_or(0, |element_id| {
            u64::try_from(element_id.len()).unwrap_or(u64::MAX)
        });
        let record_bytes = 512_u64
            .saturating_add(logical_aligned_bytes(pointer_bytes))
            .saturating_add(logical_aligned_bytes(element_id_bytes));
        self.transact(
            request,
            ResourceContext::new(phase, event, &diagnostic.json_pointer)
                .with_element_id(diagnostic.element_id.as_deref())
                .with_local_owner(diagnostic.element_id.as_deref())
                .with_record(ResourceRecordKind::Diagnostic),
            ResourceDelta {
                diagnostics: 1,
                working_bytes: record_bytes,
                total_work_units: 8,
                ..ResourceDelta::default()
            },
        )?;
        if self.report.diagnostics.try_reserve(1).is_err() {
            return Err(self.memory_failure_at(
                request,
                "compatibility_report",
                "diagnostic_occurrence",
                diagnostic.element_id,
                diagnostic.json_pointer,
            ));
        }
        self.report.diagnostics.push(diagnostic);
        Ok(())
    }

    fn measure_and_emit_output(
        &mut self,
        request: &RenderRequest,
        error_code: ErrorCode,
        codec_emission_work: u64,
        mut encode: impl FnMut() -> Result<Vec<u8>, FormatEncodeError>,
    ) -> Result<(), RenderError> {
        let measured = encode().map_err(|error| self.format_failure(request, error_code, error))?;
        let output_bytes = u64::try_from(measured.len()).unwrap_or(u64::MAX);
        self.admit_output(request, output_bytes, codec_emission_work)?;
        drop(measured);

        let emitted = encode().map_err(|error| self.format_failure(request, error_code, error))?;
        if u64::try_from(emitted.len()).unwrap_or(u64::MAX) != output_bytes {
            return Err(self.internal_failure(
                request.format(),
                "measured and emitted artifact lengths differ",
                "",
            ));
        }
        self.staged_output = Some(emitted);
        Ok(())
    }

    fn admit_output(
        &mut self,
        request: &RenderRequest,
        output_bytes: u64,
        codec_emission_work: u64,
    ) -> Result<(), RenderError> {
        let output_record_bytes = 512_u64.saturating_add(logical_aligned_bytes(output_bytes));
        let total_work_units = codec_emission_work.saturating_add(output_bytes);
        self.transact(
            request,
            ResourceContext::new(
                ResourcePhase::EncodingAndOutcomeAssembly,
                "artifact_retention",
                "",
            )
            .with_record(ResourceRecordKind::OutputBuffer),
            ResourceDelta {
                working_bytes: output_record_bytes,
                output_bytes,
                total_work_units,
                ..ResourceDelta::default()
            },
        )?;
        Ok(())
    }

    fn release_working(
        &mut self,
        request: &RenderRequest,
        context: ResourceContext<'_>,
        bytes: u64,
    ) -> Result<(), RenderError> {
        self.meter
            .release_working_bytes(context, bytes)
            .map_err(|()| {
                self.internal_failure(
                    request.format(),
                    "semantic working-byte record underflow",
                    "",
                )
            })
    }

    fn transact(
        &mut self,
        request: &RenderRequest,
        context: ResourceContext<'_>,
        delta: ResourceDelta,
    ) -> Result<(), RenderError> {
        let failure_location = match context.try_failure_location() {
            Ok(location) => location,
            Err(_) => {
                return Err(self.memory_failure(request, "resource_failure", "source_location"));
            }
        };
        match self.meter.transact(context, delta) {
            Ok(()) => Ok(()),
            Err(crossing) => Err(self.resource_failure(request, crossing, failure_location)),
        }
    }

    fn resource_failure(
        &mut self,
        request: &RenderRequest,
        crossing: ResourceCrossing,
        location: crate::resource_accounting::ResourceFailureLocation,
    ) -> RenderError {
        let (code, message, element_id, json_pointer, details) =
            crossing.into_failure_parts(request.resource_limits(), location);
        RenderError {
            category: RenderErrorCategory::ResourceLimit,
            code,
            message: Box::from(message),
            format: request.format(),
            element_id,
            json_pointer: json_pointer.into_boxed_str(),
            details: Some(Box::new(details)),
            report: std::mem::take(&mut self.report),
        }
    }

    fn validate_svg_dimensions(
        &mut self,
        resolved: &ResolvedScene,
        request: &RenderRequest,
    ) -> Result<(), RenderError> {
        let scale = match request.sizing() {
            Sizing::Scale(scale) => scale,
            Sizing::MaxDimension(_) => 1.0,
        };
        let limit = request.resource_limits().max_svg_dimension();
        for attempted in [
            resolved.logical_width,
            resolved.logical_height,
            resolved.logical_width * scale,
            resolved.logical_height * scale,
        ] {
            if attempted > limit {
                let configured = FiniteValue::new(limit).ok_or_else(|| {
                    self.internal_failure(
                        request.format(),
                        "validated SVG dimension limit is not finite",
                        "",
                    )
                })?;
                let attempted = FiniteValue::new(attempted).ok_or_else(|| {
                    self.internal_failure(
                        request.format(),
                        "attempted SVG dimension is not finite",
                        "",
                    )
                })?;
                return Err(RenderError {
                    category: RenderErrorCategory::ResourceLimit,
                    code: ErrorCode::RESOURCE_SVG_DIMENSIONS_EXCEEDED,
                    message: Box::from("SVG dimension limit exceeded"),
                    format: request.format(),
                    element_id: None,
                    json_pointer: Box::from(""),
                    details: Some(Box::new(ErrorDetails::ResourceLimit {
                        field: "max_svg_dimension",
                        configured: ResourceValue::Finite(configured),
                        value_kind: ValueKind::Dimension,
                        measurement: Measurement::Attempted {
                            value: ResourceValue::Finite(attempted),
                        },
                        recovery: None,
                    })),
                    report: std::mem::take(&mut self.report),
                });
            }
        }
        Ok(())
    }

    fn format_failure(
        &mut self,
        request: &RenderRequest,
        code: ErrorCode,
        error: FormatEncodeError,
    ) -> RenderError {
        match error {
            FormatEncodeError::Encoding(message) => {
                self.encoding_failure(request.format(), code, message)
            }
            FormatEncodeError::MemoryUnavailable {
                subject_kind,
                subject,
            } => self.memory_failure(request, subject_kind, subject),
        }
    }

    fn memory_failure(
        &mut self,
        request: &RenderRequest,
        subject_kind: &str,
        subject: &str,
    ) -> RenderError {
        self.memory_failure_at(request, subject_kind, subject, None, String::new())
    }

    fn memory_failure_at(
        &mut self,
        request: &RenderRequest,
        subject_kind: &str,
        subject: &str,
        element_id: Option<String>,
        json_pointer: String,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::ResourceLimit,
            code: ErrorCode::RESOURCE_MEMORY_UNAVAILABLE,
            message: Box::from("required physical allocation is unavailable"),
            format: request.format(),
            element_id,
            json_pointer: json_pointer.into_boxed_str(),
            details: Some(Box::new(ErrorDetails::MemoryUnavailable {
                subject_kind: subject_kind.to_owned(),
                subject: subject.to_owned(),
            })),
            report: std::mem::take(&mut self.report),
        }
    }

    fn encoding_failure(
        &mut self,
        format: OutputFormat,
        code: ErrorCode,
        message: String,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::Encoding,
            code,
            message: message.into_boxed_str(),
            format,
            element_id: None,
            json_pointer: Box::from(""),
            details: None,
            report: std::mem::take(&mut self.report),
        }
    }

    fn internal_failure(
        &mut self,
        format: OutputFormat,
        message: &'static str,
        json_pointer: &'static str,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::Internal,
            code: ErrorCode::RENDER_INTERNAL_FAILURE,
            message: Box::from(message),
            format,
            element_id: None,
            json_pointer: Box::from(json_pointer),
            details: None,
            report: std::mem::take(&mut self.report),
        }
    }
}

struct ResolvedScene {
    logical_width: f64,
    logical_height: f64,
    include_background: bool,
    background_color: String,
    elements: Vec<ResolvedElement>,
}

impl ResolvedScene {
    fn empty(request: &RenderRequest) -> Self {
        let logical_size = request.padding() * 2.0;
        Self {
            logical_width: logical_size,
            logical_height: logical_size,
            include_background: request.include_background(),
            background_color: "#ffffff".to_owned(),
            elements: Vec::new(),
        }
    }

    fn rectangle(
        source: crate::scene::RectangleSource<'_>,
        request: &RenderRequest,
        include_outline: bool,
    ) -> Result<Option<Self>, std::collections::TryReserveError> {
        if source.background_color != "transparent"
            || source.fill_style != "solid"
            || source.stroke_style != "solid"
            || source.stroke_color != "#1e1e1e"
            || source.view_background_color.unwrap_or("#ffffff") != "#ffffff"
            || source.stroke_width < 0.0
            || source.roughness < 0.0
            || !(0.0..=100.0).contains(&source.opacity)
            || source.seed == 0
        {
            return Ok(None);
        }

        let mut x = source.x;
        let mut y = source.y;
        let mut width = source.width;
        let mut height = source.height;
        if width < 0.0 {
            x += width;
            width = -width;
        }
        if height < 0.0 {
            y += height;
            height = -height;
        }
        if width == 0.0 || height == 0.0 {
            return Ok(None);
        }

        let center_x = x + width / 2.0;
        let center_y = y + height / 2.0;
        let sin = source.angle.sin();
        let cos = source.angle.cos();
        let corners = [
            (x, y),
            (x + width, y),
            (x + width, y + height),
            (x, y + height),
        ];
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (corner_x, corner_y) in corners {
            let delta_x = corner_x - center_x;
            let delta_y = corner_y - center_y;
            let rotated_x = center_x + delta_x * cos - delta_y * sin;
            let rotated_y = center_y + delta_x * sin + delta_y * cos;
            min_x = min_x.min(rotated_x);
            min_y = min_y.min(rotated_y);
            max_x = max_x.max(rotated_x);
            max_y = max_y.max(rotated_y);
        }
        let logical_width = max_x - min_x + request.padding() * 2.0;
        let logical_height = max_y - min_y + request.padding() * 2.0;
        if !logical_width.is_finite() || !logical_height.is_finite() {
            return Ok(None);
        }

        let max_size = width.max(height);
        let min_size = width.min(height);
        let adjusted_roughness = if min_size >= 20.0 && max_size >= 50.0 {
            source.roughness
        } else {
            (source.roughness / if max_size < 10.0 { 3.0 } else { 2.0 }).min(2.5)
        };
        let translation_x = x - min_x + request.padding();
        let translation_y = y - min_y + request.padding();
        let mut elements = Vec::new();
        if include_outline {
            let path = rough::rectangle(width, height, source.seed, adjusted_roughness)?;
            elements.try_reserve_exact(1)?;
            elements.push(ResolvedElement::Rectangle(ResolvedRectangle {
                source_id: try_owned(source.id)?,
                source_pointer: "/elements/0",
                path,
                transform: ResolvedTransform {
                    translation_x,
                    translation_y,
                    angle_radians: source.angle,
                    center_x: width / 2.0,
                    center_y: height / 2.0,
                },
                stroke_color: try_owned(source.stroke_color)?,
                stroke_width: source.stroke_width,
                opacity: source.opacity / 100.0,
            }));
        }

        Ok(Some(Self {
            logical_width,
            logical_height,
            include_background: request.include_background(),
            background_color: try_owned(source.view_background_color.unwrap_or("#ffffff"))?,
            elements,
        }))
    }

    fn primary_source(&self) -> Option<(&str, &str)> {
        self.elements.first().map(|element| match element {
            ResolvedElement::Rectangle(rectangle) => {
                (rectangle.source_id.as_str(), rectangle.source_pointer)
            }
        })
    }

    fn raster_or_pdf_scale(&self, sizing: Sizing) -> f64 {
        let largest_dimension = self.logical_width.max(self.logical_height);
        match sizing {
            Sizing::Scale(scale) => scale,
            Sizing::MaxDimension(maximum) if maximum < largest_dimension => {
                maximum / largest_dimension
            }
            Sizing::MaxDimension(_) => 1.0,
        }
    }
}

enum ResolvedElement {
    Rectangle(ResolvedRectangle),
}

struct ResolvedRectangle {
    source_id: String,
    source_pointer: &'static str,
    path: rough::Path,
    transform: ResolvedTransform,
    stroke_color: String,
    stroke_width: f64,
    opacity: f64,
}

struct ResolvedTransform {
    translation_x: f64,
    translation_y: f64,
    angle_radians: f64,
    center_x: f64,
    center_y: f64,
}

struct RectangleLocalCrossing {
    code: DiagnosticCode,
    field: &'static str,
    configured: ResourceValue,
    value_kind: ValueKind,
    attempted: ResourceValue,
    json_pointer: &'static str,
    message: &'static str,
}

impl RectangleLocalCrossing {
    fn diagnostic(
        self,
        format: OutputFormat,
        element_id: &str,
    ) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
        let mut effects = Vec::new();
        effects.try_reserve_exact(1)?;
        effects.push(DiagnosticEffect::Visual);
        Ok(CompatibilityDiagnostic {
            code: self.code,
            severity: DiagnosticSeverity::Warning,
            format,
            effects,
            element_id: Some(try_owned(element_id)?),
            json_pointer: try_owned(self.json_pointer)?,
            message: try_owned(self.message)?,
            details: Some(Box::new(ErrorDetails::ResourceLimit {
                field: self.field,
                configured: self.configured,
                value_kind: self.value_kind,
                measurement: Measurement::Attempted {
                    value: self.attempted,
                },
                recovery: Some(Recovery::Omitted),
            })),
        })
    }
}

fn rectangle_local_crossing(
    source: crate::scene::RectangleSource<'_>,
    request: &RenderRequest,
    geometry_segments: u64,
    element_work_units: u64,
) -> Option<RectangleLocalCrossing> {
    let coordinate_limit = request.resource_limits().max_coordinate_magnitude();
    for (value, pointer) in [
        (source.x, "/elements/0/x"),
        (source.y, "/elements/0/y"),
        (source.width, "/elements/0/width"),
        (source.height, "/elements/0/height"),
    ] {
        let magnitude = value.abs();
        if magnitude > coordinate_limit {
            return Some(RectangleLocalCrossing {
                code: DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED,
                field: "max_coordinate_magnitude",
                configured: ResourceValue::Finite(FiniteValue::new(coordinate_limit)?),
                value_kind: ValueKind::Coordinate,
                attempted: ResourceValue::Finite(FiniteValue::new(magnitude)?),
                json_pointer: pointer,
                message: "rectangle omitted because a coordinate exceeds its limit",
            });
        }
    }

    let geometry_limit = u64::from(
        request
            .resource_limits()
            .max_geometry_segments_per_element(),
    );
    if geometry_segments > geometry_limit {
        return Some(RectangleLocalCrossing {
            code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
            field: "max_geometry_segments_per_element",
            configured: ResourceValue::Integer(geometry_limit),
            value_kind: ValueKind::Count,
            attempted: ResourceValue::Integer(geometry_segments),
            json_pointer: "/elements/0",
            message: "rectangle omitted because its geometry exceeds its limit",
        });
    }

    let work_limit = request.resource_limits().max_element_work_units();
    (element_work_units > work_limit).then_some(RectangleLocalCrossing {
        code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
        field: "max_element_work_units",
        configured: ResourceValue::Integer(work_limit),
        value_kind: ValueKind::WorkUnits,
        attempted: ResourceValue::Integer(element_work_units),
        json_pointer: "/elements/0",
        message: "rectangle omitted because its semantic work exceeds its limit",
    })
}

fn logical_aligned_bytes(bytes: u64) -> u64 {
    bytes.saturating_add(63) / 64 * 64
}

fn owned_field_bytes(value: &str) -> u64 {
    64_u64.saturating_add(logical_aligned_bytes(
        u64::try_from(value.len()).unwrap_or(u64::MAX),
    ))
}

fn try_owned(value: &str) -> Result<String, std::collections::TryReserveError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len())?;
    owned.push_str(value);
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompatibilityDiagnostic, CompatibilityRevision, DiagnosticCode, DiagnosticEffect,
        DiagnosticSeverity, Renderer, ResourceLimits,
    };
    use serde_json::json;

    const EMPTY_SCENE: &[u8] =
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;

    #[test]
    fn diagnostic_transaction_uses_fatal_precedence_and_never_appends_a_rejection() {
        let cases = [
            (1, 640, 8, ErrorCode::RESOURCE_DIAGNOSTIC_COUNT_EXCEEDED),
            (2, 640, 8, ErrorCode::RESOURCE_WORKING_BYTES_EXCEEDED),
            (2, 1_280, 8, ErrorCode::RESOURCE_TOTAL_WORK_EXCEEDED),
        ];

        for (max_diagnostics, max_working_bytes, max_total_work_units, expected_code) in cases {
            let limits = ResourceLimits::builder()
                .max_diagnostics(max_diagnostics)
                .max_working_bytes(max_working_bytes)
                .max_element_work_units(1)
                .max_asset_work_units(1)
                .max_total_work_units(max_total_work_units)
                .build()
                .expect("diagnostic precedence limits are valid");
            let request = RenderRequest::builder(OutputFormat::Svg)
                .resource_limits(limits)
                .build()
                .expect("diagnostic request is valid");
            let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
                .expect("revision 1 exists");
            let mut session = RenderSession::new(profile, &request);
            let diagnostic = test_diagnostic();

            session
                .retain_diagnostic(
                    &request,
                    ResourcePhase::JsonTraversalAndNormalization,
                    "test_diagnostic",
                    diagnostic.clone(),
                )
                .expect("the first diagnostic fits at equality");
            let error = session
                .retain_diagnostic(
                    &request,
                    ResourcePhase::JsonTraversalAndNormalization,
                    "test_diagnostic",
                    diagnostic.clone(),
                )
                .expect_err("the second diagnostic crosses an aggregate limit");

            assert_eq!(error.code(), expected_code);
            assert_eq!(error.report().diagnostics(), &[diagnostic]);
        }
    }

    #[test]
    fn later_output_exhaustion_preserves_the_accumulated_report() {
        let limits = ResourceLimits::builder()
            .max_output_bytes(1)
            .build()
            .expect("output limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("request is valid");
        let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");
        let mut session = RenderSession::new(profile, &request);
        let diagnostic = test_diagnostic();
        session
            .retain_diagnostic(
                &request,
                ResourcePhase::ProjectionAndFallback,
                "test_diagnostic",
                diagnostic.clone(),
            )
            .expect("diagnostic is retained");

        let error = session
            .render(&scene, &request)
            .expect_err("output exhaustion is fatal");

        assert_eq!(error.code(), ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED);
        assert_eq!(error.report().diagnostics(), &[diagnostic]);
    }

    #[test]
    fn diagnostic_trace_uses_the_callers_canonical_phase() {
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("request is valid");
        let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");
        let mut session = RenderSession::new_with_trace(profile, &scene, &request);

        session
            .retain_diagnostic(
                &request,
                ResourcePhase::JsonTraversalAndNormalization,
                "normalization_diagnostic",
                test_diagnostic(),
            )
            .expect("diagnostic is retained");

        let trace: serde_json::Value =
            serde_json::from_slice(&session.meter.trace_bytes()).expect("trace is valid JSON");
        assert_eq!(
            trace["entries"][0]["phase"],
            "json_traversal_and_normalization"
        );
        assert_eq!(trace["entries"][0]["event"], "normalization_diagnostic");
    }

    #[test]
    fn canonical_resource_trace_is_uncharged_stable_and_semantic() {
        let expected_digests = [
            (
                OutputFormat::Svg,
                "82d3cd30e8fbcb015fd0124c95d17652425093c97ea91cd199ff57bbccbfd72f",
            ),
            (
                OutputFormat::Png,
                "b1fba20f0164fb87923913f43df945630c18e93f168fc32d2552b68793c46aaf",
            ),
            (
                OutputFormat::Pdf,
                "2fe91836dcd24a658d49d86449ebeb5384168b36c3a95be883d0abb1c965e336",
            ),
        ];

        for (format, expected_digest) in expected_digests {
            let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
            let request = RenderRequest::builder(format)
                .build()
                .expect("request is valid");
            let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
                .expect("revision 1 exists");
            let ordinary = Renderer::new()
                .render(&scene, &request)
                .expect("ordinary render succeeds");

            let (first_result, first_trace) = render_with_trace(profile, &scene, &request);
            let (second_result, second_trace) = render_with_trace(profile, &scene, &request);
            let first = first_result.expect("traced render succeeds");
            let second = second_result.expect("repeated traced render succeeds");

            assert_eq!(first_trace, second_trace);
            assert_eq!(first.artifact().bytes(), ordinary.artifact().bytes());
            assert_eq!(second.artifact().bytes(), ordinary.artifact().bytes());
            assert_eq!(first.report(), ordinary.report());
            assert_eq!(second.report(), ordinary.report());

            let trace_text = std::str::from_utf8(&first_trace).expect("trace is UTF-8 JSON");
            for forbidden in ["allocator", "backend", "cache", "thread", "timing"] {
                assert!(
                    !trace_text.contains(forbidden),
                    "trace exposes forbidden implementation fact {forbidden}"
                );
            }
            let trace: serde_json::Value =
                serde_json::from_slice(&first_trace).expect("trace schema is valid JSON");
            assert_eq!(trace["schema_version"], 1);
            assert_eq!(trace["header"]["compatibility_revision"], 1);
            assert_eq!(trace["header"]["format"], format.as_str());
            assert_eq!(trace["header"]["limits"]["max_scene_bytes"], 33_554_432);
            assert_eq!(
                trace["header"]["input_digests"]["supplemental_assets"],
                json!([])
            );

            let entries = trace["entries"]
                .as_array()
                .expect("trace entries are an array");
            assert!(entries.iter().all(|entry| {
                matches!(
                    entry["decision"].as_str(),
                    Some(
                        "committed"
                            | "rejected_local"
                            | "rejected_fatal"
                            | "shared_intrinsic"
                            | "released"
                    )
                )
            }));
            assert_eq!(entries[0]["event"], "request_session");
            assert_eq!(entries[1]["event"], "scene_source_octets");
            assert_eq!(entries[2]["event"], "json_node");
            assert_eq!(entries[2]["json_pointer"], "");

            let digest = crate::resource_accounting::sha256_hex(&first_trace);
            assert_eq!(
                digest, expected_digest,
                "canonical {format:?} trace changed"
            );
        }
    }

    #[test]
    fn rejected_trace_entry_is_exact_and_does_not_commit_partial_output() {
        let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
        let limits = ResourceLimits::builder()
            .max_output_bytes(1)
            .build()
            .expect("output limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("request is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");

        let (result, trace_bytes) = render_with_trace(profile, &scene, &request);
        let error = result.expect_err("output exhaustion returns no artifact");
        assert_eq!(error.code(), ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED);
        assert!(error.report().is_empty());

        let trace: serde_json::Value =
            serde_json::from_slice(&trace_bytes).expect("trace is valid JSON");
        let entries = trace["entries"].as_array().expect("entries are an array");
        let rejected = entries.last().expect("trace has a rejected transaction");
        assert_eq!(rejected["event"], "artifact_retention");
        assert_eq!(rejected["decision"], "rejected_fatal");
        assert_eq!(rejected["code"], "resource.output-bytes-exceeded");
        assert_eq!(rejected["measurement"], "exact");
        assert_eq!(rejected["before"], rejected["after"]);
        assert_eq!(rejected["after"]["output_bytes"], 0);
        assert!(rejected["attempted"]["output_bytes"]
            .as_u64()
            .is_some_and(|attempted| attempted > 1));
    }

    #[test]
    fn rejected_output_measurement_never_runs_the_emission_pass() {
        let limits = ResourceLimits::builder()
            .max_output_bytes(1)
            .build()
            .expect("output limit is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .resource_limits(limits)
            .build()
            .expect("request is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");
        let mut session = RenderSession::new(profile, &request);
        let mut passes = 0;

        let error = session
            .measure_and_emit_output(&request, ErrorCode::ENCODE_SVG_FAILED, 0, || {
                passes += 1;
                Ok(b"measured output".to_vec())
            })
            .expect_err("measured output crosses the configured ceiling");

        assert_eq!(error.code(), ErrorCode::RESOURCE_OUTPUT_BYTES_EXCEEDED);
        assert_eq!(passes, 1, "rejected output is measured but never emitted");
        assert!(session.staged_output.is_none());
    }

    fn test_diagnostic() -> CompatibilityDiagnostic {
        CompatibilityDiagnostic {
            code: DiagnosticCode::COLOR_INVALID,
            severity: DiagnosticSeverity::Warning,
            format: OutputFormat::Svg,
            effects: vec![DiagnosticEffect::Visual],
            element_id: Some("element-1".to_owned()),
            json_pointer: "/elements/0/color".to_owned(),
            message: "invalid color substituted".to_owned(),
            details: None,
        }
    }
}
