mod pdf;
mod png;
mod rough;
mod svg;

use crate::{
    catalog::RevisionCatalog,
    normalization::{
        NormalizationError, NormalizationEvent, NormalizedElementSource, NormalizedRectangle,
    },
    outcome::DiagnosticOrder,
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

const RECTANGLE_GEOMETRY_SEGMENTS: u64 = 16;
const RECTANGLE_GEOMETRY_WORK_UNITS: u64 = RECTANGLE_GEOMETRY_SEGMENTS * 32;

pub(super) enum FormatEncodeError {
    Encoding(String),
    MemoryUnavailable {
        subject_kind: &'static str,
        subject: &'static str,
    },
}

pub(crate) fn render(
    catalog: &'static RevisionCatalog,
    scene: &SceneDocument,
    request: &RenderRequest,
) -> Result<RenderOutcome, RenderError> {
    let profile = catalog.profile();
    debug_assert_eq!(
        catalog.asset_catalog().compatibility_revision,
        profile.compatibility_revision().get()
    );
    debug_assert_eq!(
        catalog.asset_catalog().reference_profile_digest,
        profile.profile_digest()
    );
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

        let mut semantic_working = Vec::new();
        let resolved = if scene.has_no_element_occurrences() {
            ResolvedScene::empty(request)
        } else {
            let normalization = crate::normalization::normalize_rectangles(
                scene,
                self.profile.compatibility_revision(),
                request.format(),
                |event| match event {
                    NormalizationEvent::PreflightRecord {
                        json_pointer,
                        working_bytes,
                        work_units,
                    } => {
                        let delta = ResourceDelta {
                            working_bytes,
                            total_work_units: work_units,
                            ..ResourceDelta::default()
                        };
                        if self.meter.would_cross_fatal(delta) {
                            let context = ResourceContext::new(
                                ResourcePhase::JsonTraversalAndNormalization,
                                "normalized_rectangle",
                                json_pointer,
                            );
                            return match self.transact(request, context, delta) {
                                Err(error) => Err(error),
                                Ok(()) => Err(self.internal_failure(
                                    request.format(),
                                    "normalization preflight committed unexpectedly",
                                    "/elements",
                                )),
                            };
                        }
                        Ok(())
                    }
                    NormalizationEvent::CommitRecord {
                        element_id,
                        json_pointer,
                        working_bytes,
                        work_units,
                    } => self.transact(
                        request,
                        ResourceContext::new(
                            ResourcePhase::JsonTraversalAndNormalization,
                            "normalized_rectangle",
                            json_pointer,
                        )
                        .with_element_id(Some(element_id))
                        .with_local_owner(Some(element_id)),
                        ResourceDelta {
                            working_bytes,
                            total_work_units: work_units,
                            ..ResourceDelta::default()
                        },
                    ),
                    NormalizationEvent::Diagnostic(diagnostic) => self.retain_diagnostic(
                        request,
                        ResourcePhase::JsonTraversalAndNormalization,
                        "normalization_repair",
                        diagnostic,
                    ),
                },
            );
            let normalization = match normalization {
                Ok(normalization) => normalization,
                Err(NormalizationError::Observer(error)) => return Err(error),
                Err(NormalizationError::MemoryUnavailable { source_index }) => {
                    let json_pointer = source_index
                        .and_then(|index| crate::scene::pointer_index("/elements", index).ok())
                        .unwrap_or_else(|| "/elements".to_owned());
                    return Err(self.memory_failure_at(
                        request,
                        "normalization",
                        "rectangle",
                        None,
                        json_pointer,
                    ));
                }
            }
            .ok_or_else(|| {
                self.internal_failure(
                    request.format(),
                    "nonempty scene is outside the implemented rectangle slice",
                    "/elements",
                )
            })?;
            semantic_working
                .try_reserve_exact(normalization.rectangles.len())
                .map_err(|_| self.memory_failure(request, "normalization", "working_records"))?;
            let mut rectangles = Vec::new();
            rectangles
                .try_reserve_exact(normalization.rectangles.len())
                .map_err(|_| self.memory_failure(request, "normalization", "rectangle_list"))?;
            for source in normalization.rectangles {
                let has_outline = source.width != 0.0 && source.height != 0.0;
                let geometry_segments = if has_outline {
                    RECTANGLE_GEOMETRY_SEGMENTS
                } else {
                    0
                };
                let geometry_work_units = geometry_segments * 32;
                let geometry_working_bytes = if has_outline {
                    128 + geometry_segments * 64
                } else {
                    0
                };
                let geometry_delta = ResourceDelta {
                    geometry_segments,
                    working_bytes: geometry_working_bytes,
                    total_work_units: geometry_work_units,
                    ..ResourceDelta::default()
                };
                let geometry_context = ResourceContext::new(
                    ResourcePhase::SharedSemanticResolution,
                    "rough_rectangle_geometry",
                    source.source.pointer(),
                )
                .with_element_id(Some(source.source.id()))
                .with_local_owner(Some(source.source.id()));
                if self.meter.would_cross_fatal(geometry_delta) {
                    return match self.transact(request, geometry_context, geometry_delta) {
                        Err(error) => Err(error),
                        Ok(()) => Err(self.internal_failure(
                            request.format(),
                            "fatal resource preflight committed unexpectedly",
                            "/elements",
                        )),
                    };
                }
                let element_work_units = source.normalization_work_units
                    + geometry_work_units
                    + 8
                    + source.repair_work_units;
                let local_crossing = rectangle_local_crossing(
                    &source,
                    request,
                    geometry_segments,
                    element_work_units,
                );
                let include_outline = has_outline && local_crossing.is_none();
                let semantic_working_bytes = if let Some(crossing) = local_crossing {
                    let diagnostic = crossing
                        .diagnostic(
                            request.format(),
                            source.source.id(),
                            source.source.pointer(),
                        )
                        .map_err(|_| {
                            self.memory_failure(request, "compatibility_report", "local_recovery")
                        })?;
                    self.retain_diagnostic(
                        request,
                        ResourcePhase::SharedSemanticResolution,
                        "rectangle_local_resource_recovery",
                        diagnostic,
                    )?;
                    source.normalized_working_bytes
                } else if has_outline {
                    self.transact(request, geometry_context, geometry_delta)?;
                    source
                        .normalized_working_bytes
                        .saturating_add(geometry_working_bytes)
                } else {
                    source.normalized_working_bytes
                };
                semantic_working.push(SemanticWorkingRecord {
                    element_id: try_owned(source.source.id()).map_err(|_| {
                        self.memory_failure(request, "normalization", "working_record_identity")
                    })?,
                    json_pointer: try_owned(source.source.pointer()).map_err(|_| {
                        self.memory_failure(request, "normalization", "working_record_pointer")
                    })?,
                    bytes: semantic_working_bytes,
                });
                rectangles.push((source, include_outline));
            }
            if rectangles.is_empty() {
                ResolvedScene::empty(request)
            } else {
                match ResolvedScene::rectangles(rectangles, request) {
                    Ok(Some(resolved)) => resolved,
                    Ok(None) => {
                        return Err(self.internal_failure(
                            request.format(),
                            "rectangle semantics are outside the implemented focused case",
                            "/elements",
                        ));
                    }
                    Err(_) => {
                        return Err(self.memory_failure(request, "semantic_geometry", "rectangle"));
                    }
                }
            }
        };
        let bytes = self.encode(resolved, request, &semantic_working)?;

        Ok(RenderOutcome {
            artifact: Artifact {
                format: request.format(),
                bytes,
            },
            report: self.take_report(),
            profile: self.profile,
        })
    }

    fn encode(
        &mut self,
        resolved: ResolvedScene,
        request: &RenderRequest,
        semantic_working: &[SemanticWorkingRecord],
    ) -> Result<Vec<u8>, RenderError> {
        match request.format() {
            OutputFormat::Svg => self.validate_svg_dimensions(&resolved, request)?,
            OutputFormat::Pdf => self.validate_pdf_dimensions(&resolved, request)?,
            OutputFormat::Png => {}
        }
        let plan_base = ResourceContext::new(ResourcePhase::FormatPlanning, "format_plan_base", "")
            .with_record(ResourceRecordKind::FormatPlanBase);
        self.acquire_working(request, plan_base, 1_024)?;
        let pdf_profile_selection = if request.format() == OutputFormat::Pdf {
            let selection = ResourceContext::new(
                ResourcePhase::FormatPlanning,
                "pdf_srgb_profile_selection",
                "",
            )
            .with_record(ResourceRecordKind::BuiltInSelectionState);
            self.acquire_working(request, selection, 256)?;
            Some(selection)
        } else {
            None
        };
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
        for index in 0..resolved.source_count() {
            let (element_id, json_pointer) = resolved.source(index).ok_or_else(|| {
                self.internal_failure(
                    request.format(),
                    "resolved source operation is missing",
                    "/elements",
                )
            })?;
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
        match request.format() {
            OutputFormat::Svg => {
                let plan = SvgPlan::new(resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_SVG_FAILED, error)
                })?;
                self.release_semantic_working(request, semantic_working)?;
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
                for index in 0..plan.source_count() {
                    let (element_id, json_pointer) = plan.source(index).ok_or_else(|| {
                        self.internal_failure(
                            request.format(),
                            "SVG source operation is missing",
                            "/elements",
                        )
                    })?;
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
                let mut plan = PngPlan::new(resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_PNG_FAILED, error)
                })?;
                self.release_semantic_working(request, semantic_working)?;
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
                for index in 0..plan.source_count() {
                    let vector_work = plan
                        .vector_coverage_area(index)
                        .ok_or_else(|| {
                            self.internal_failure(
                                request.format(),
                                "PNG vector coverage source is missing",
                                "/elements",
                            )
                        })?
                        .saturating_mul(16);
                    let (element_id, json_pointer) = plan.source(index).ok_or_else(|| {
                        self.internal_failure(
                            request.format(),
                            "PNG source operation is missing",
                            "/elements",
                        )
                    })?;
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
                    let element_work = plan
                        .base_element_work_units(index)
                        .ok_or_else(|| {
                            self.internal_failure(
                                request.format(),
                                "PNG element work source is missing",
                                "/elements",
                            )
                        })?
                        .saturating_add(vector_work);
                    let element_work_limit = request.resource_limits().max_element_work_units();
                    if element_work > element_work_limit {
                        let diagnostic = RectangleLocalCrossing {
                            code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
                            field: "max_element_work_units",
                            configured: ResourceValue::Integer(element_work_limit),
                            value_kind: ValueKind::WorkUnits,
                            attempted: ResourceValue::Integer(element_work),
                            pointer_field: None,
                            message:
                                "rectangle omitted because its semantic work exceeds its limit",
                        }
                        .diagnostic(request.format(), element_id, json_pointer)
                        .map_err(|_| {
                            self.memory_failure(request, "compatibility_report", "local_recovery")
                        })?;
                        self.retain_diagnostic(
                            request,
                            ResourcePhase::ProjectionAndFallback,
                            "rectangle_local_resource_recovery",
                            diagnostic,
                        )?;
                        if !plan.omit_element(index) {
                            return Err(self.internal_failure(
                                request.format(),
                                "PNG omitted source operation is missing",
                                "/elements",
                            ));
                        }
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
                for index in 0..plan.source_count() {
                    let (element_id, json_pointer) = plan.source(index).ok_or_else(|| {
                        self.internal_failure(
                            request.format(),
                            "PNG source operation is missing",
                            "/elements",
                        )
                    })?;
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
                let plan = PdfPlan::new(resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_PDF_FAILED, error)
                })?;
                self.release_semantic_working(request, semantic_working)?;
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
                for index in 0..plan.source_count() {
                    let (element_id, json_pointer) = plan.source(index).ok_or_else(|| {
                        self.internal_failure(
                            request.format(),
                            "PDF source operation is missing",
                            "/elements",
                        )
                    })?;
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
        }

        if let Some(profile_selection) = pdf_profile_selection {
            self.release_working(request, profile_selection, 256)?;
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
        mut diagnostic: CompatibilityDiagnostic,
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
        diagnostic.order = DiagnosticOrder {
            owner_index: element_owner_index(&diagnostic.json_pointer).unwrap_or(u64::MAX),
            retention_ordinal: u64::try_from(self.report.diagnostics.len()).unwrap_or(u64::MAX),
        };
        self.report.diagnostics.push(diagnostic);
        Ok(())
    }

    fn take_report(&mut self) -> CompatibilityReport {
        self.report.diagnostics.sort_unstable_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.json_pointer.cmp(&right.json_pointer))
                .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        });
        std::mem::take(&mut self.report)
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

    fn release_semantic_working(
        &mut self,
        request: &RenderRequest,
        records: &[SemanticWorkingRecord],
    ) -> Result<(), RenderError> {
        for record in records {
            self.release_working(
                request,
                ResourceContext::new(
                    ResourcePhase::FormatPlanning,
                    "resolved_rectangle",
                    &record.json_pointer,
                )
                .with_element_id(Some(&record.element_id))
                .with_local_owner(Some(&record.element_id)),
                record.bytes,
            )?;
        }
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
            report: self.take_report(),
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
                    report: self.take_report(),
                });
            }
        }
        Ok(())
    }

    fn validate_pdf_dimensions(
        &mut self,
        resolved: &ResolvedScene,
        request: &RenderRequest,
    ) -> Result<(), RenderError> {
        let limit = request.resource_limits().max_pdf_page_points();
        let scale = resolved.raster_or_pdf_scale(request.sizing());
        for logical in [resolved.logical_width, resolved.logical_height] {
            let attempted = logical * scale * pdf::POINTS_PER_LOGICAL_PIXEL;
            if attempted > limit {
                let configured = FiniteValue::new(limit).ok_or_else(|| {
                    self.internal_failure(
                        request.format(),
                        "validated PDF page limit is not finite",
                        "",
                    )
                })?;
                let measurement = if let Some(attempted) = FiniteValue::new(attempted) {
                    Measurement::Attempted {
                        value: ResourceValue::Finite(attempted),
                    }
                } else {
                    let lower_bound = FiniteValue::new(limit + 1.0).ok_or_else(|| {
                        self.internal_failure(
                            request.format(),
                            "PDF page crossing lower bound is not finite",
                            "",
                        )
                    })?;
                    Measurement::AtLeast {
                        value: ResourceValue::Finite(lower_bound),
                    }
                };
                return Err(RenderError {
                    category: RenderErrorCategory::ResourceLimit,
                    code: ErrorCode::RESOURCE_PDF_PAGE_DIMENSIONS_EXCEEDED,
                    message: Box::from("PDF page point limit exceeded"),
                    format: request.format(),
                    element_id: None,
                    json_pointer: Box::from(""),
                    details: Some(Box::new(ErrorDetails::ResourceLimit {
                        field: "max_pdf_page_points",
                        configured: ResourceValue::Finite(configured),
                        value_kind: ValueKind::Dimension,
                        measurement,
                        recovery: None,
                    })),
                    report: self.take_report(),
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
            report: self.take_report(),
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
            report: self.take_report(),
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
            report: self.take_report(),
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

    fn rectangles(
        rectangles: Vec<(NormalizedRectangle, bool)>,
        request: &RenderRequest,
    ) -> Result<Option<Self>, std::collections::TryReserveError> {
        if rectangles.is_empty() {
            return Ok(None);
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (source, _) in &rectangles {
            if source.background_color != "transparent"
                || source.fill_style != "solid"
                || source.stroke_style != "solid"
                || source.stroke_color != "#1e1e1e"
                || source.view_background_color != "#ffffff"
                || source.stroke_width < 0.0
                || source.roughness < 0.0
                || !(0.0..=100.0).contains(&source.opacity)
                || source.seed == 0
            {
                return Ok(None);
            }
            let center_x = source.x + source.width / 2.0;
            let center_y = source.y + source.height / 2.0;
            let sin = source.angle.sin();
            let cos = source.angle.cos();
            for (corner_x, corner_y) in [
                (source.x, source.y),
                (source.x + source.width, source.y),
                (source.x + source.width, source.y + source.height),
                (source.x, source.y + source.height),
            ] {
                let delta_x = corner_x - center_x;
                let delta_y = corner_y - center_y;
                let rotated_x = center_x + delta_x * cos - delta_y * sin;
                let rotated_y = center_y + delta_x * sin + delta_y * cos;
                min_x = min_x.min(rotated_x);
                min_y = min_y.min(rotated_y);
                max_x = max_x.max(rotated_x);
                max_y = max_y.max(rotated_y);
            }
        }
        let logical_width = max_x - min_x + request.padding() * 2.0;
        let logical_height = max_y - min_y + request.padding() * 2.0;
        if !logical_width.is_finite() || !logical_height.is_finite() {
            return Ok(None);
        }

        let mut elements = Vec::new();
        elements.try_reserve_exact(rectangles.len())?;
        let background_color = rectangles
            .first()
            .map(|(source, _)| source.view_background_color.as_str())
            .unwrap_or("#ffffff");
        let background_color = try_owned(background_color)?;
        for (source, include_outline) in rectangles {
            if !include_outline {
                continue;
            }
            let max_size = source.width.max(source.height);
            let min_size = source.width.min(source.height);
            let adjusted_roughness = if min_size >= 20.0 && max_size >= 50.0 {
                source.roughness
            } else {
                (source.roughness / if max_size < 10.0 { 3.0 } else { 2.0 }).min(2.5)
            };
            let path =
                rough::rectangle(source.width, source.height, source.seed, adjusted_roughness)?;
            elements.push(ResolvedElement::Rectangle(ResolvedRectangle {
                source: source.source,
                base_element_work_units: source.normalization_work_units
                    + RECTANGLE_GEOMETRY_WORK_UNITS
                    + 8
                    + source.repair_work_units,
                path,
                transform: ResolvedTransform {
                    translation_x: source.x - min_x + request.padding(),
                    translation_y: source.y - min_y + request.padding(),
                    angle_radians: source.angle,
                    center_x: source.width / 2.0,
                    center_y: source.height / 2.0,
                },
                stroke_color: source.stroke_color,
                stroke_width: source.stroke_width,
                opacity: source.opacity / 100.0,
            }));
        }

        Ok(Some(Self {
            logical_width,
            logical_height,
            include_background: request.include_background(),
            background_color,
            elements,
        }))
    }

    fn source_count(&self) -> usize {
        self.elements.len()
    }

    fn source(&self, index: usize) -> Option<(&str, &str)> {
        self.elements.get(index).map(|element| match element {
            ResolvedElement::Rectangle(rectangle) => {
                (rectangle.source.id(), rectangle.source.pointer())
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
    source: NormalizedElementSource,
    base_element_work_units: u64,
    path: rough::Path,
    transform: ResolvedTransform,
    stroke_color: String,
    stroke_width: f64,
    opacity: f64,
}

struct SemanticWorkingRecord {
    element_id: String,
    json_pointer: String,
    bytes: u64,
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
    pointer_field: Option<&'static str>,
    message: &'static str,
}

impl RectangleLocalCrossing {
    fn diagnostic(
        self,
        format: OutputFormat,
        element_id: &str,
        element_pointer: &str,
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
            json_pointer: self.pointer_field.map_or_else(
                || try_owned(element_pointer),
                |field| crate::scene::pointer_child(element_pointer, field),
            )?,
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
            order: DiagnosticOrder::default(),
        })
    }
}

fn rectangle_local_crossing(
    source: &NormalizedRectangle,
    request: &RenderRequest,
    geometry_segments: u64,
    element_work_units: u64,
) -> Option<RectangleLocalCrossing> {
    let coordinate_limit = request.resource_limits().max_coordinate_magnitude();
    for (value, pointer_field) in [
        (source.x, "x"),
        (source.y, "y"),
        (source.width, "width"),
        (source.height, "height"),
    ] {
        let magnitude = value.abs();
        if magnitude > coordinate_limit {
            return Some(RectangleLocalCrossing {
                code: DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED,
                field: "max_coordinate_magnitude",
                configured: ResourceValue::Finite(FiniteValue::new(coordinate_limit)?),
                value_kind: ValueKind::Coordinate,
                attempted: ResourceValue::Finite(FiniteValue::new(magnitude)?),
                pointer_field: Some(pointer_field),
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
            pointer_field: None,
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
        pointer_field: None,
        message: "rectangle omitted because its semantic work exceeds its limit",
    })
}

fn logical_aligned_bytes(bytes: u64) -> u64 {
    bytes.saturating_add(63) / 64 * 64
}

fn element_owner_index(json_pointer: &str) -> Option<u64> {
    json_pointer
        .strip_prefix("/elements/")?
        .split('/')
        .next()?
        .parse()
        .ok()
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
    use std::fs;
    use std::path::Path;

    const EMPTY_SCENE: &[u8] =
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;

    #[test]
    fn focused_pdf_expectations_are_approval_bound() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate is inside the workspace");
        let path = root.join(concat!(
            "tests/conformance/cases/focused/rectangle-outline/",
            "pdf-renderer-expectations.json"
        ));
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(path).expect("PDF expectation manifest exists"))
                .expect("PDF expectation manifest is valid JSON");
        assert_eq!(manifest["case_id"], "focused.rectangle-outline.v1");
        assert_eq!(manifest["format"], "pdf");
        assert_eq!(manifest["compatibility_revision"], 1);
        assert_eq!(manifest["variant"], "default");
        assert_eq!(manifest["regions"], json!(["whole"]));
        assert_eq!(manifest["visual_class"], "geometry/image");
        assert_eq!(manifest["crops"], json!([]));
        assert_eq!(manifest["coverage"].as_array().map(Vec::len), Some(4));
        assert_eq!(
            manifest["case_manifest_approval_subject_sha256"],
            "sha256:90f12e04953edec55aa051d0abf8a49c28da5a2e6d9a28d4e2b57196775f4698"
        );

        let approval = manifest
            .as_object_mut()
            .expect("PDF expectation manifest is an object")
            .remove("approval")
            .expect("PDF expectation manifest is approved");
        assert_eq!(approval["approved_by"], "sagikazarmark");
        assert_eq!(approval["approved_at"], "2026-07-20");
        assert_eq!(
            approval["approval_url"],
            "https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5027454753"
        );
        let mut canonical = serde_json::to_vec(&manifest).expect("unsigned manifest serializes");
        canonical.push(b'\n');
        assert_eq!(
            approval["approval_subject_sha256"],
            format!(
                "sha256:{}",
                crate::resource_accounting::sha256_hex(&canonical)
            )
        );

        let files = manifest["files"]
            .as_array()
            .expect("PDF expectation files are an array");
        assert_eq!(files.len(), 13);
        for descriptor in files {
            let path = descriptor["path"]
                .as_str()
                .expect("PDF expectation path is a string");
            let bytes = fs::read(root.join(path)).expect("PDF expectation dependency exists");
            assert_eq!(
                descriptor["sha256"],
                crate::resource_accounting::sha256_hex(&bytes),
                "stale PDF expectation dependency {path}"
            );
        }

        let exception: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join(concat!(
                "tests/conformance/cases/focused/rectangle-outline/",
                "pdf-visual-exception.json"
            )))
            .expect("PDF visual exception exists"),
        )
        .expect("PDF visual exception is valid JSON");
        let keys: Vec<_> = exception
            .as_object()
            .expect("PDF visual exception is an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "approved_issue",
                "authority",
                "case_id",
                "format",
                "max_changed_pixels",
                "minimum_ssim_bits",
                "reason",
                "region",
                "schema",
                "variant",
                "viewer_profile",
                "visual_class",
            ]
        );
        assert_eq!(exception["case_id"], "focused.rectangle-outline.v1");
        assert_eq!(exception["variant"], "default");
        assert_eq!(exception["format"], "pdf");
        assert_eq!(exception["viewer_profile"], "canonical-pdf.pdfium.v1");
        assert_eq!(exception["region"], "whole");
        assert_eq!(exception["visual_class"], "geometry/image");
        assert_eq!(exception["max_changed_pixels"], 0);
        assert_eq!(exception["minimum_ssim_bits"], "3fefec69adc2369e");
    }

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
                "29fdab71a03d23839b02fc361575610d541fa7d84169946b7eb0f59b4798fa15",
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
    fn normalization_resource_events_repeat_with_generated_identity_and_source_order() {
        let source = br#"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"#;
        let scene = SceneDocument::parse(source).expect("normalization vector is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("request is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");

        let (first_result, first_trace) = render_with_trace(profile, &scene, &request);
        let (second_result, second_trace) = render_with_trace(profile, &scene, &request);
        let first = first_result.expect("normalization vector renders");
        let second = second_result.expect("repeated normalization vector renders");

        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        assert_eq!(first_trace, second_trace);

        let trace: serde_json::Value =
            serde_json::from_slice(&first_trace).expect("trace is valid JSON");
        let normalization_entries = trace["entries"]
            .as_array()
            .expect("trace entries are an array")
            .iter()
            .filter(|entry| {
                matches!(
                    entry["event"].as_str(),
                    Some("normalization_repair" | "normalized_rectangle")
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(normalization_entries.len(), 4);
        assert_eq!(
            normalization_entries
                .iter()
                .map(|entry| entry["json_pointer"].as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![
                "/elements/0",
                "/elements/0/id",
                "/elements/0/seed",
                "/elements/0/updated",
            ]
        );
        assert!(normalization_entries.iter().all(|entry| {
            entry["element_id"]
                == "generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792"
        }));
        assert!(normalization_entries[1..].iter().all(|entry| {
            entry["deltas"]["diagnostics"] == 1 && entry["deltas"]["total_work_units"] == 8
        }));
        assert_eq!(
            normalization_entries[0]["deltas"]["total_work_units"],
            crate::normalization::NORMALIZATION_WORK_UNITS
        );
    }

    #[test]
    fn format_plan_and_semantic_records_keep_each_rectangle_owner() {
        let source = br#"{"type":"excalidraw","version":2,"elements":[{"id":"first","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"id":"second","type":"rectangle","x":50,"width":40,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"#;
        let scene = SceneDocument::parse(source).expect("multi-rectangle vector is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("request is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");

        let (result, trace) = render_with_trace(profile, &scene, &request);
        result.expect("multi-rectangle vector renders");
        let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
        let operations = trace["entries"]
            .as_array()
            .expect("trace entries are an array")
            .iter()
            .filter(|entry| {
                entry["event"] == "element_operation" && entry["decision"] == "committed"
            })
            .map(|entry| {
                (
                    entry["element_id"].as_str().unwrap_or_default(),
                    entry["json_pointer"].as_str().unwrap_or_default(),
                    entry["deltas"]["total_work_units"]
                        .as_u64()
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            operations,
            vec![("first", "/elements/0", 8), ("second", "/elements/1", 8),]
        );
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
            order: DiagnosticOrder::default(),
        }
    }
}
