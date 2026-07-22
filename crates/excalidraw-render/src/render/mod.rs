mod freedraw;
mod pdf;
mod png;
mod rough;
mod svg;
mod text;

use crate::{
    catalog::RevisionCatalog,
    color::{self, CanonicalColor},
    normalization::{
        NormalizationError, NormalizationEvent, NormalizedElement, NormalizedElementKind,
        NormalizedElementSource,
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

fn surface_diagnostic_preflight_limit(request: &RenderRequest) -> usize {
    let limits = request.resource_limits();
    let diagnostic_limit = u64::from(limits.max_diagnostics()).saturating_add(1);
    let working_limit = limits
        .max_working_bytes()
        .saturating_div(512)
        .saturating_add(1);
    let work_limit = limits
        .max_total_work_units()
        .saturating_div(8)
        .saturating_add(1);
    usize::try_from(diagnostic_limit.min(working_limit).min(work_limit)).unwrap_or(usize::MAX)
}

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
    let mut session = RenderSession::new(profile, catalog.scene_surface(), scene, request);
    session.render(scene, request)
}

#[cfg(test)]
fn render_with_trace(
    profile: &'static ReferenceProfileIdentity,
    scene: &SceneDocument,
    request: &RenderRequest,
) -> (Result<RenderOutcome, RenderError>, Vec<u8>) {
    let mut session = RenderSession::new_with_trace(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        scene,
        request,
    );
    let result = session.render(scene, request);
    (result, session.meter.trace_bytes())
}

struct RenderSession {
    profile: &'static ReferenceProfileIdentity,
    scene_surface: &'static crate::scene_surface::SceneSurfacePolicy,
    report: CompatibilityReport,
    meter: ResourceMeter,
    staged_output: Option<Vec<u8>>,
    element_source_order_base: u64,
    excalifont_selected: bool,
    excalifont_range_selected: bool,
    excalifont_glyphs: Vec<u16>,
    excalifont_request_derived_bytes: u64,
}

#[derive(Clone, Copy, Default)]
struct DiagnosticOwnership<'a> {
    local_owner: Option<&'a str>,
    source_order: Option<u64>,
    source_position: Option<u64>,
}

impl RenderSession {
    fn new(
        profile: &'static ReferenceProfileIdentity,
        scene_surface: &'static crate::scene_surface::SceneSurfacePolicy,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Self {
        Self {
            profile,
            scene_surface,
            report: CompatibilityReport::empty(),
            meter: ResourceMeter::new(request.resource_limits()),
            staged_output: None,
            element_source_order_base: scene.source_owner_order("elements", 0),
            excalifont_selected: false,
            excalifont_range_selected: false,
            excalifont_glyphs: Vec::new(),
            excalifont_request_derived_bytes: 0,
        }
    }

    #[cfg(test)]
    fn new_with_trace(
        profile: &'static ReferenceProfileIdentity,
        scene_surface: &'static crate::scene_surface::SceneSurfacePolicy,
        scene: &SceneDocument,
        request: &RenderRequest,
    ) -> Self {
        Self {
            profile,
            scene_surface,
            report: CompatibilityReport::empty(),
            meter: ResourceMeter::with_trace(profile, scene, request),
            staged_output: None,
            element_source_order_base: scene.source_owner_order("elements", 0),
            excalifont_selected: false,
            excalifont_range_selected: false,
            excalifont_glyphs: Vec::new(),
            excalifont_request_derived_bytes: 0,
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
        let mut resolved = {
            let normalization = crate::normalization::normalize_scene_with_assets(
                scene,
                self.profile.compatibility_revision(),
                self.scene_surface,
                request.format(),
                request.supplemental_assets(),
                request.frame_options().enabled() && request.frame_options().name(),
                surface_diagnostic_preflight_limit(request),
                |event| match event {
                    NormalizationEvent::PreflightRecord {
                        event,
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
                                event,
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
                        event,
                        element_id,
                        local_owner,
                        json_pointer,
                        working_bytes,
                        work_units,
                    } => self.transact(
                        request,
                        ResourceContext::new(
                            ResourcePhase::JsonTraversalAndNormalization,
                            event,
                            json_pointer,
                        )
                        .with_element_id(element_id)
                        .with_local_owner(local_owner),
                        ResourceDelta {
                            working_bytes,
                            total_work_units: work_units,
                            ..ResourceDelta::default()
                        },
                    ),
                    NormalizationEvent::Diagnostic {
                        diagnostic,
                        local_owner,
                        source_order,
                        source_position,
                    } => self.retain_diagnostic_with_owner(
                        request,
                        ResourcePhase::JsonTraversalAndNormalization,
                        "normalization_repair",
                        diagnostic,
                        DiagnosticOwnership {
                            local_owner,
                            source_order: Some(source_order),
                            source_position: Some(source_position),
                        },
                    ),
                },
            );
            let normalization = match normalization {
                Ok(normalization) => normalization,
                Err(NormalizationError::Observer(error)) => return Err(error),
                Err(NormalizationError::MemoryUnavailable { source_index }) => {
                    let json_pointer = source_index
                        .and_then(|index| crate::scene::pointer_index("/elements", index).ok())
                        .unwrap_or_default();
                    return Err(self.memory_failure_at(
                        request,
                        "normalization",
                        "record",
                        None,
                        json_pointer,
                    ));
                }
            }
            .ok_or_else(|| {
                self.internal_failure(
                    request.format(),
                    "nonempty scene is outside the implemented outline slice",
                    "/elements",
                )
            })?;
            let relationships = crate::relationships::resolve(
                scene,
                &normalization.elements,
                &normalization.files,
                request.supplemental_assets(),
                |event| match event {
                    crate::relationships::RelationshipEvent::SearchStep {
                        element_id,
                        json_pointer,
                        attempted_depth,
                    } => {
                        let context = ResourceContext::new(
                            ResourcePhase::SharedSemanticResolution,
                            "relationship_search_step",
                            json_pointer,
                        )
                        .with_element_id(Some(element_id))
                        .with_local_owner(Some(element_id));
                        let delta = ResourceDelta {
                            observed_relationship_depth: u64::from(attempted_depth),
                            total_work_units: crate::relationships::traversal_delta(false).0,
                            ..ResourceDelta::default()
                        };
                        self.transact(request, context, delta)
                    }
                    crate::relationships::RelationshipEvent::Traversal(traversal) => {
                        let (total_work_units, working_bytes) =
                            crate::relationships::traversal_delta(traversal.retained);
                        self.transact(
                            request,
                            ResourceContext::new(
                                ResourcePhase::SharedSemanticResolution,
                                "relationship_traversal",
                                traversal.json_pointer,
                            )
                            .with_element_id(Some(traversal.element_id))
                            .with_local_owner(Some(traversal.local_owner)),
                            ResourceDelta {
                                observed_relationship_depth: u64::from(traversal.attempted_depth),
                                working_bytes,
                                total_work_units,
                                ..ResourceDelta::default()
                            },
                        )
                    }
                },
            );
            let relationships = match relationships {
                Ok(relationships) => relationships,
                Err(crate::relationships::RelationshipError::Observer(error)) => return Err(error),
                Err(crate::relationships::RelationshipError::MemoryUnavailable) => {
                    return Err(self.memory_failure(
                        request,
                        "relationship_resolution",
                        "relationship_graph",
                    ));
                }
            };
            for cycle in &relationships.removed_cycles {
                let diagnostic = relationship_cycle_diagnostic(
                    request.format(),
                    &cycle.element_id,
                    &cycle.json_pointer,
                )
                .map_err(|_| {
                    self.memory_failure(request, "compatibility_report", "relationship_cycle")
                })?;
                self.retain_diagnostic_with_owner(
                    request,
                    ResourcePhase::SharedSemanticResolution,
                    "relationship_cycle_removal",
                    diagnostic,
                    DiagnosticOwnership {
                        local_owner: Some(&cycle.element_id),
                        source_order: Some(cycle.source_order),
                        source_position: Some(cycle.source_position),
                    },
                )?;
            }
            let semantic_record_count = normalization
                .elements
                .len()
                .saturating_add(normalization.fallbacks.len())
                .saturating_add(normalization.files.len())
                .saturating_add(relationships.graph.edges.len())
                .saturating_add(1);
            semantic_working
                .try_reserve_exact(semantic_record_count)
                .map_err(|_| self.memory_failure(request, "normalization", "working_records"))?;
            let frame_rendering = normalization.frame_rendering;
            let view_background_color = normalization.view_background_color;
            let view_background_working_bytes = normalization.view_background_working_bytes;
            let effective_frame_rendering = EffectiveFrameRendering {
                enabled: frame_rendering.enabled && request.frame_options().enabled(),
                clip: frame_rendering.clip && request.frame_options().clip(),
                name: frame_rendering.name && request.frame_options().name(),
                outline: frame_rendering.outline && request.frame_options().outline(),
            };
            let normalized_files = normalization.files;
            for edge in &relationships.graph.edges {
                let owner = relationships.graph.nodes.get(edge.source).ok_or_else(|| {
                    self.internal_failure(
                        request.format(),
                        "relationship edge owner is missing",
                        "/elements",
                    )
                })?;
                semantic_working.push(SemanticWorkingRecord {
                    source_order: owner.source_order,
                    event: "relationship_edge",
                    element_id: Some(try_owned(&owner.id).map_err(|_| {
                        self.memory_failure(request, "relationship_resolution", "edge_identity")
                    })?),
                    local_owner: Some(try_owned(&owner.id).map_err(|_| {
                        self.memory_failure(request, "relationship_resolution", "edge_owner")
                    })?),
                    json_pointer: try_owned(&edge.json_pointer).map_err(|_| {
                        self.memory_failure(request, "relationship_resolution", "edge_pointer")
                    })?,
                    bytes: crate::relationships::edge_working_bytes(),
                });
            }
            let mut fallbacks = normalization.fallbacks;
            let mut outlines = Vec::new();
            outlines
                .try_reserve_exact(normalization.elements.len())
                .map_err(|_| self.memory_failure(request, "normalization", "outline_list"))?;
            for source in normalization.elements {
                debug_assert!(!source.index.is_empty());
                let is_outline = matches!(
                    &source.kind,
                    NormalizedElementKind::Rectangle
                        | NormalizedElementKind::Diamond
                        | NormalizedElementKind::Ellipse
                        | NormalizedElementKind::Linear(crate::normalization::NormalizedLinear {
                            geometry_available: true,
                            ..
                        })
                        | NormalizedElementKind::Freedraw(
                            crate::normalization::NormalizedFreedraw {
                                geometry_available: true,
                                ..
                            }
                        )
                );
                if matches!(
                    source.kind,
                    NormalizedElementKind::Frame(_) | NormalizedElementKind::MagicFrame(_)
                ) {
                    semantic_working.push(SemanticWorkingRecord {
                        source_order: source.source_order,
                        event: "resolved_frame",
                        element_id: Some(try_owned(source.source.id()).map_err(|_| {
                            self.memory_failure(request, "normalization", "working_record_identity")
                        })?),
                        local_owner: Some(try_owned(source.source.id()).map_err(|_| {
                            self.memory_failure(request, "normalization", "working_record_owner")
                        })?),
                        json_pointer: try_owned(source.source.pointer()).map_err(|_| {
                            self.memory_failure(request, "normalization", "working_record_pointer")
                        })?,
                        bytes: source.normalized_working_bytes,
                    });
                    outlines.push((source, None));
                    continue;
                }
                if matches!(&source.kind, NormalizedElementKind::Text(_)) {
                    if self.preflight_official_text(&source, request, &mut semantic_working)? {
                        outlines.push((source, None));
                    }
                    continue;
                }
                if !is_outline || !source.direct_outline_compatible {
                    let uses_non_generic_recovery =
                        self.report.diagnostics.iter().any(|diagnostic| {
                            diagnostic_uses_non_generic_recovery(&source, diagnostic)
                        });
                    if !self.report.diagnostics.iter().any(|diagnostic| {
                        diagnostic.element_id() == Some(source.source.id())
                            && diagnostic_replaces_element(diagnostic)
                    }) {
                        let diagnostic = known_element_replaced_diagnostic(
                            request.format(),
                            source.source.id(),
                            source.source.pointer(),
                            source.width > 0.0 && source.height > 0.0,
                        )
                        .map_err(|_| {
                            self.memory_failure(
                                request,
                                "compatibility_report",
                                "known_element_replacement",
                            )
                        })?;
                        self.retain_diagnostic_with_owner(
                            request,
                            ResourcePhase::SharedSemanticResolution,
                            "known_element_replacement",
                            diagnostic,
                            DiagnosticOwnership {
                                local_owner: Some(source.source.id()),
                                source_order: Some(source.source_order),
                                source_position: Some(0),
                            },
                        )?;
                    }
                    semantic_working.push(SemanticWorkingRecord {
                        source_order: source.source_order,
                        event: "resolved_known_element_fallback",
                        element_id: Some(try_owned(source.source.id()).map_err(|_| {
                            self.memory_failure(request, "normalization", "working_record_identity")
                        })?),
                        local_owner: Some(try_owned(source.source.id()).map_err(|_| {
                            self.memory_failure(request, "normalization", "working_record_owner")
                        })?),
                        json_pointer: try_owned(source.source.pointer()).map_err(|_| {
                            self.memory_failure(request, "normalization", "working_record_pointer")
                        })?,
                        bytes: source.normalized_working_bytes,
                    });
                    if source.width > 0.0 && source.height > 0.0 {
                        fallbacks.try_reserve(1).map_err(|_| {
                            self.memory_failure(request, "semantic_geometry", "fallback_list")
                        })?;
                        let mut fallback = known_element_fallback(&source).map_err(|_| {
                            self.memory_failure(
                                request,
                                "semantic_geometry",
                                "known_element_fallback",
                            )
                        })?;
                        fallback.paint = !uses_non_generic_recovery;
                        fallbacks.push(fallback);
                    }
                    continue;
                }
                let has_outline = match &source.kind {
                    NormalizedElementKind::Linear(linear) => linear
                        .points
                        .windows(2)
                        .any(|points| points[0] != points[1]),
                    NormalizedElementKind::Freedraw(freedraw) => {
                        !freedraw.points.is_empty() || !freedraw.simulate_pressure
                    }
                    _ => source.width != 0.0 && source.height != 0.0,
                };
                let base_element_work_units = source
                    .normalization_work_units
                    .saturating_add(8)
                    .saturating_add(source.repair_work_units);
                // Coordinate and already-spent work admission must precede any Rough allocation.
                let mut local_crossing =
                    outline_local_crossing(&source, request, 0, base_element_work_units);
                let geometry = if has_outline && local_crossing.is_none() {
                    let max_size = source.width.max(source.height);
                    let min_size = source.width.min(source.height);
                    let adjusted_roughness = if min_size >= 20.0 && max_size >= 50.0
                        || min_size >= 15.0 && source.roundness.is_some()
                        || matches!(source.kind, NormalizedElementKind::Linear(_))
                            && max_size >= 50.0
                    {
                        source.roughness
                    } else {
                        (source.roughness / if max_size < 10.0 { 3.0 } else { 2.0 }).min(2.5)
                    };
                    let mut budget = rough::GenerationBudget::new(
                        u64::from(
                            request
                                .resource_limits()
                                .max_geometry_segments_per_element(),
                        ),
                        request.resource_limits().max_element_work_units(),
                        base_element_work_units,
                    );
                    let generated = match &source.kind {
                        NormalizedElementKind::Freedraw(freedraw) => {
                            freedraw::generate(freedraw, source.stroke_width, &mut budget).and_then(
                                |path| {
                                    let background_fill =
                                        if source.background_color == "transparent" {
                                            None
                                        } else {
                                            let fill_style = match source.fill_style.as_str() {
                                                "hachure" => rough::FillStyle::Hachure,
                                                "cross-hatch" => rough::FillStyle::CrossHatch,
                                                "zigzag" => rough::FillStyle::Zigzag,
                                                _ => rough::FillStyle::Solid,
                                            };
                                            freedraw::background_fill(
                                                &freedraw.points,
                                                source.seed,
                                                adjusted_roughness,
                                                fill_style,
                                                source.stroke_width,
                                                &mut budget,
                                            )?
                                        };
                                    Ok(GeneratedGeometry::Freedraw(GeneratedFreedraw {
                                        path,
                                        background_fill,
                                    }))
                                },
                            )
                        }
                        _ => rough_drawing(&source, adjusted_roughness, &mut budget)
                            .map(GeneratedGeometry::Rough),
                    };
                    match generated {
                        Ok(geometry) => Some(geometry),
                        Err(rough::GenerationError::Memory) => {
                            return Err(self.memory_failure(
                                request,
                                "semantic_geometry",
                                "rough_outline",
                            ));
                        }
                        Err(rough::GenerationError::Limit { kind, attempted }) => {
                            local_crossing =
                                Some(generation_local_crossing(kind, attempted, request));
                            None
                        }
                    }
                } else {
                    None
                };
                let geometry_segments = geometry.as_ref().map_or(0, |geometry| {
                    u64::try_from(geometry.operation_count()).unwrap_or(u64::MAX)
                });
                let geometry_work_units = geometry_segments * 32;
                let geometry_working_bytes = if geometry.is_some() {
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
                let geometry_event = match source.kind {
                    NormalizedElementKind::Rectangle => "rough_rectangle_geometry",
                    NormalizedElementKind::Diamond => "rough_diamond_geometry",
                    NormalizedElementKind::Ellipse => "rough_ellipse_geometry",
                    NormalizedElementKind::Linear(_) => "rough_linear_geometry",
                    NormalizedElementKind::Freedraw(_) => "freedraw_geometry",
                    NormalizedElementKind::Text(_)
                    | NormalizedElementKind::Image(_)
                    | NormalizedElementKind::Frame(_)
                    | NormalizedElementKind::MagicFrame(_) => {
                        unreachable!("only direct outlines reach geometry accounting")
                    }
                };
                let geometry_context = ResourceContext::new(
                    ResourcePhase::SharedSemanticResolution,
                    geometry_event,
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
                let element_work_units =
                    base_element_work_units.saturating_add(geometry_work_units);
                if local_crossing.is_none() {
                    local_crossing = outline_local_crossing(
                        &source,
                        request,
                        geometry_segments,
                        element_work_units,
                    );
                }
                let include_outline = has_outline && local_crossing.is_none();
                let semantic_working_bytes = if let Some(crossing) = local_crossing {
                    let source_position = crossing
                        .pointer_field
                        .map_or(u64::MAX, |field| source.source.coordinate_position(field));
                    let diagnostic = crossing
                        .diagnostic(
                            request.format(),
                            Some(source.source.id()),
                            source.source.pointer(),
                        )
                        .map_err(|_| {
                            self.memory_failure(request, "compatibility_report", "local_recovery")
                        })?;
                    self.retain_diagnostic_with_owner(
                        request,
                        ResourcePhase::SharedSemanticResolution,
                        "outline_local_resource_recovery",
                        diagnostic,
                        DiagnosticOwnership {
                            local_owner: Some(source.source.id()),
                            source_order: Some(source.source_order),
                            source_position: Some(source_position),
                        },
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
                    source_order: source.source_order,
                    event: "resolved_outline",
                    element_id: Some(try_owned(source.source.id()).map_err(|_| {
                        self.memory_failure(request, "normalization", "working_record_identity")
                    })?),
                    local_owner: Some(try_owned(source.source.id()).map_err(|_| {
                        self.memory_failure(request, "normalization", "working_record_owner")
                    })?),
                    json_pointer: try_owned(source.source.pointer()).map_err(|_| {
                        self.memory_failure(request, "normalization", "working_record_pointer")
                    })?,
                    bytes: semantic_working_bytes,
                });
                outlines.push((source, include_outline.then_some(geometry).flatten()));
            }
            semantic_working.push(SemanticWorkingRecord {
                source_order: scene.source_owner_order("appState", 0),
                event: "normalized_frame_rendering",
                element_id: None,
                local_owner: None,
                json_pointer: "/appState/frameRendering".to_owned(),
                bytes: 4 * 64 + view_background_working_bytes,
            });
            for file in normalized_files {
                semantic_working.push(SemanticWorkingRecord {
                    source_order: file.source_order,
                    event: "normalized_file_record",
                    element_id: None,
                    local_owner: Some(file.key),
                    json_pointer: file.source_pointer,
                    bytes: file.normalized_working_bytes,
                });
            }
            self.account_compatibility_omissions(request)?;
            self.preflight_fallback_text(&mut fallbacks, request, &mut semantic_working)?;
            semantic_working.sort_unstable_by_key(|record| record.source_order);
            if outlines.is_empty() && fallbacks.is_empty() {
                match ResolvedScene::empty(relationships.graph, &view_background_color, request) {
                    Ok(resolved) => resolved,
                    Err(ResolveSceneError::UnboundedGeometry) => {
                        return Err(self.unbounded_geometry_failure(request.format()));
                    }
                    Err(ResolveSceneError::InvalidColor) => {
                        return Err(self.internal_failure(
                            request.format(),
                            "normalized scene background color is invalid",
                            "/appState/viewBackgroundColor",
                        ));
                    }
                    Err(ResolveSceneError::Memory | ResolveSceneError::InvalidFallbackFont) => {
                        return Err(self.internal_failure(
                            request.format(),
                            "empty scene resolution reached an impossible failure",
                            "",
                        ));
                    }
                }
            } else {
                match ResolvedScene::elements(
                    outlines,
                    fallbacks,
                    relationships.graph,
                    effective_frame_rendering,
                    &view_background_color,
                    request,
                ) {
                    Ok(Some(resolved)) => resolved,
                    Ok(None) => {
                        return Err(self.internal_failure(
                            request.format(),
                            "outline semantics are outside the implemented focused case",
                            "/elements",
                        ));
                    }
                    Err(ResolveSceneError::Memory) => {
                        return Err(self.memory_failure(request, "semantic_geometry", "outline"));
                    }
                    Err(ResolveSceneError::InvalidFallbackFont) => {
                        return Err(self.internal_failure(
                            request.format(),
                            "the sealed fallback font could not be resolved",
                            "/elements",
                        ));
                    }
                    Err(ResolveSceneError::InvalidColor) => {
                        return Err(self.internal_failure(
                            request.format(),
                            "normalized scene color is invalid",
                            "/elements",
                        ));
                    }
                    Err(ResolveSceneError::UnboundedGeometry) => {
                        return Err(self.unbounded_geometry_failure(request.format()));
                    }
                }
            }
        };
        self.account_fallback_semantics(&mut resolved, request, &mut semantic_working)?;
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
                .with_element_id(element_id)
                .with_local_owner(element_id)
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
                        .with_element_id(element_id)
                        .with_local_owner(element_id)
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
                let frame_mask_bytes = plan
                    .maximum_frame_mask_surface_area()
                    .saturating_add(u64::from(plan.has_frame_masks()) * 256);
                let frame_mask_surface = ResourceContext::new(
                    ResourcePhase::ProjectionAndFallback,
                    "frame_clip_mask_surface",
                    "",
                )
                .with_record(ResourceRecordKind::RasterMaskSurface);
                if frame_mask_bytes != 0 {
                    self.acquire_working(request, frame_mask_surface, frame_mask_bytes)?;
                }

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
                    let (element_id, json_pointer) = plan.source(index).ok_or_else(|| {
                        self.internal_failure(
                            request.format(),
                            "PNG source operation is missing",
                            "/elements",
                        )
                    })?;
                    if let Some(mask_pixels) =
                        plan.frame_mask_surface_area(index).ok_or_else(|| {
                            self.internal_failure(
                                request.format(),
                                "PNG frame mask source is missing",
                                "/elements",
                            )
                        })?
                    {
                        let surface_limit = request.resource_limits().max_raster_surface_pixels();
                        let mask_delta = ResourceDelta {
                            rasterized_pixels: mask_pixels,
                            total_work_units: mask_pixels.saturating_mul(2),
                            ..ResourceDelta::default()
                        };
                        let mask_context = ResourceContext::new(
                            ResourcePhase::ProjectionAndFallback,
                            "frame_clip_mask_surface",
                            json_pointer,
                        )
                        .with_element_id(element_id)
                        .with_local_owner(element_id)
                        .with_record(ResourceRecordKind::RasterMaskSurface);
                        if self.meter.would_cross_fatal(mask_delta) {
                            self.transact(request, mask_context, mask_delta)?;
                            return Err(self.internal_failure(
                                request.format(),
                                "fatal frame mask preflight committed unexpectedly",
                                "/elements",
                            ));
                        }
                        if mask_pixels > surface_limit {
                            let diagnostic = ElementLocalCrossing {
                                code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
                                field: "max_raster_surface_pixels",
                                configured: ResourceValue::Integer(surface_limit),
                                value_kind: ValueKind::Pixels,
                                attempted: ResourceValue::Integer(mask_pixels),
                                pointer_field: None,
                                message: "framed element omitted because its clip mask exceeds its raster surface limit",
                            }
                            .diagnostic(request.format(), element_id, json_pointer)
                            .map_err(|_| {
                                self.memory_failure(
                                    request,
                                    "compatibility_report",
                                    "frame_mask_surface_recovery",
                                )
                            })?;
                            self.retain_diagnostic(
                                request,
                                ResourcePhase::ProjectionAndFallback,
                                "frame_mask_surface_local_recovery",
                                diagnostic,
                            )?;
                            self.charge_local_omission(
                                request,
                                element_id,
                                json_pointer,
                                "frame_mask_surface_local_omission",
                            )?;
                            if !plan.omit_element(index) {
                                return Err(self.internal_failure(
                                    request.format(),
                                    "PNG omitted framed operation is missing",
                                    "/elements",
                                ));
                            }
                            continue;
                        }
                        self.transact(request, mask_context, mask_delta)?;
                    }
                    if let Some(fallback_pixels) =
                        plan.fallback_surface_area(index).ok_or_else(|| {
                            self.internal_failure(
                                request.format(),
                                "PNG fallback surface source is missing",
                                "/elements",
                            )
                        })?
                    {
                        let surface_limit = request.resource_limits().max_raster_surface_pixels();
                        let raster_delta = ResourceDelta {
                            rasterized_pixels: fallback_pixels,
                            ..ResourceDelta::default()
                        };
                        let raster_context = ResourceContext::new(
                            ResourcePhase::ProjectionAndFallback,
                            "fallback_raster_surface",
                            json_pointer,
                        )
                        .with_element_id(element_id)
                        .with_local_owner(element_id);
                        if self.meter.would_cross_fatal(raster_delta) {
                            self.transact(request, raster_context, raster_delta)?;
                            return Err(self.internal_failure(
                                request.format(),
                                "fatal fallback raster preflight committed unexpectedly",
                                "/elements",
                            ));
                        }
                        if fallback_pixels > surface_limit {
                            let diagnostic = ElementLocalCrossing {
                                code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
                                field: "max_raster_surface_pixels",
                                configured: ResourceValue::Integer(surface_limit),
                                value_kind: ValueKind::Pixels,
                                attempted: ResourceValue::Integer(fallback_pixels),
                                pointer_field: None,
                                message: "compatibility fallback omitted because its raster surface exceeds its limit",
                            }
                            .diagnostic(request.format(), element_id, json_pointer)
                            .map_err(|_| {
                                self.memory_failure(
                                    request,
                                    "compatibility_report",
                                    "fallback_surface_recovery",
                                )
                            })?;
                            self.retain_diagnostic(
                                request,
                                ResourcePhase::ProjectionAndFallback,
                                "fallback_surface_local_recovery",
                                diagnostic,
                            )?;
                            self.charge_work(
                                request,
                                ResourceContext::new(
                                    ResourcePhase::ProjectionAndFallback,
                                    "fallback_surface_local_omission",
                                    json_pointer,
                                )
                                .with_element_id(element_id)
                                .with_local_owner(element_id),
                                8,
                            )?;
                            if !plan.omit_element(index) {
                                return Err(self.internal_failure(
                                    request.format(),
                                    "PNG omitted fallback operation is missing",
                                    "/elements",
                                ));
                            }
                            continue;
                        }
                        self.transact(request, raster_context, raster_delta)?;
                    }
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
                    let context = ResourceContext::new(
                        ResourcePhase::ProjectionAndFallback,
                        "vector_coverage_samples",
                        json_pointer,
                    )
                    .with_element_id(element_id)
                    .with_local_owner(element_id);
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
                        let diagnostic = ElementLocalCrossing {
                            code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
                            field: "max_element_work_units",
                            configured: ResourceValue::Integer(element_work_limit),
                            value_kind: ValueKind::WorkUnits,
                            attempted: ResourceValue::Integer(element_work),
                            pointer_field: None,
                            message: "outline omitted because its semantic work exceeds its limit",
                        }
                        .diagnostic(request.format(), element_id, json_pointer)
                        .map_err(|_| {
                            self.memory_failure(request, "compatibility_report", "local_recovery")
                        })?;
                        self.retain_diagnostic(
                            request,
                            ResourcePhase::ProjectionAndFallback,
                            "outline_local_resource_recovery",
                            diagnostic,
                        )?;
                        self.charge_work(
                            request,
                            ResourceContext::new(
                                ResourcePhase::ProjectionAndFallback,
                                "png_element_local_omission",
                                json_pointer,
                            )
                            .with_element_id(element_id)
                            .with_local_owner(element_id),
                            8,
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
                if frame_mask_bytes != 0 {
                    self.release_working(request, frame_mask_surface, frame_mask_bytes)?;
                }
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
                        .with_element_id(element_id)
                        .with_local_owner(element_id)
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
                        .with_element_id(element_id)
                        .with_local_owner(element_id)
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

    fn account_fallback_semantics(
        &mut self,
        resolved: &mut ResolvedScene,
        request: &RenderRequest,
        semantic_working: &mut Vec<SemanticWorkingRecord>,
    ) -> Result<(), RenderError> {
        for element in &mut resolved.elements {
            let direct_clip_source = match element {
                ResolvedElement::Outline(outline) if !outline.frame_clips.is_empty() => Some((
                    outline.source.id(),
                    outline.source.pointer(),
                    &outline.frame_clips,
                )),
                ResolvedElement::Freedraw(freedraw) if !freedraw.frame_clips.is_empty() => Some((
                    freedraw.source.id(),
                    freedraw.source.pointer(),
                    &freedraw.frame_clips,
                )),
                _ => None,
            };
            if let Some((element_id, json_pointer, clips)) = direct_clip_source {
                let geometry_segments = clips.iter().fold(0_u64, |total, clip| {
                    total.saturating_add(
                        u64::try_from(clip.path.operations.len()).unwrap_or(u64::MAX),
                    )
                });
                self.transact(
                    request,
                    ResourceContext::new(
                        ResourcePhase::SharedSemanticResolution,
                        "frame_clip_geometry",
                        json_pointer,
                    )
                    .with_element_id(Some(element_id))
                    .with_local_owner(Some(element_id)),
                    ResourceDelta {
                        geometry_segments,
                        total_work_units: geometry_segments.saturating_mul(32),
                        ..ResourceDelta::default()
                    },
                )?;
                continue;
            }
            if let ResolvedElement::Text(text) = element {
                let clip_segments = text.frame_clips.iter().fold(0_u64, |total, clip| {
                    total.saturating_add(
                        u64::try_from(clip.path.operations.len()).unwrap_or(u64::MAX),
                    )
                });
                let outline_segments = text.text.outline_segments();
                let geometry_segments = outline_segments.saturating_add(clip_segments);
                let glyphs = text.text.glyph_count();
                let text_scalars = text.text.scalar_count;
                let line_break_steps = text.text.line_break_steps;
                let remaining_work_units = line_break_steps
                    .saturating_sub(text.text.prepaid_line_break_steps)
                    .saturating_mul(32)
                    .saturating_add(
                        glyphs
                            .saturating_sub(text.text.prepaid_glyphs)
                            .saturating_mul(32),
                    )
                    .saturating_add(geometry_segments.saturating_mul(32));
                let total_work_units = text_scalars
                    .saturating_mul(32)
                    .saturating_add(text.text.prepaid_glyphs.saturating_mul(32))
                    .saturating_add(text.text.prepaid_line_break_steps.saturating_mul(32))
                    .saturating_add(remaining_work_units);
                text.base_element_work_units = total_work_units;
                let path_headers = u64::try_from(text.text.lines.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(u64::try_from(text.frame_clips.len()).unwrap_or(u64::MAX));
                let working_bytes = glyphs
                    .saturating_mul(64)
                    .saturating_add(line_break_steps.saturating_mul(32))
                    .saturating_add(path_headers.saturating_mul(128))
                    .saturating_add(geometry_segments.saturating_mul(64));
                let delta = ResourceDelta {
                    geometry_segments,
                    working_bytes,
                    total_work_units: remaining_work_units,
                    ..ResourceDelta::default()
                };
                let context = ResourceContext::new(
                    ResourcePhase::SharedSemanticResolution,
                    "text_layout_and_glyph_geometry",
                    text.source.pointer(),
                )
                .with_element_id(Some(text.source.id()))
                .with_local_owner(Some(text.source.id()));
                if self.meter.would_cross_fatal(delta) {
                    self.transact(request, context, delta)?;
                    return Err(self.internal_failure(
                        request.format(),
                        "fatal text resource preflight committed unexpectedly",
                        "/elements",
                    ));
                }
                let local_crossing = if geometry_segments
                    > u64::from(
                        request
                            .resource_limits()
                            .max_geometry_segments_per_element(),
                    ) {
                    Some(ElementLocalCrossing {
                        code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
                        field: "max_geometry_segments_per_element",
                        configured: ResourceValue::Integer(u64::from(
                            request
                                .resource_limits()
                                .max_geometry_segments_per_element(),
                        )),
                        value_kind: ValueKind::Count,
                        attempted: ResourceValue::Integer(geometry_segments),
                        pointer_field: None,
                        message:
                            "text was omitted because its glyph outlines exceed the geometry limit",
                    })
                } else if total_work_units > request.resource_limits().max_element_work_units() {
                    Some(ElementLocalCrossing {
                        code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
                        field: "max_element_work_units",
                        configured: ResourceValue::Integer(
                            request.resource_limits().max_element_work_units(),
                        ),
                        value_kind: ValueKind::WorkUnits,
                        attempted: ResourceValue::Integer(total_work_units),
                        pointer_field: None,
                        message: "text was omitted because shaping exceeds its work limit",
                    })
                } else {
                    None
                };
                if let Some(crossing) = local_crossing {
                    let diagnostic = crossing
                        .diagnostic(
                            request.format(),
                            Some(text.source.id()),
                            text.source.pointer(),
                        )
                        .map_err(|_| {
                            self.memory_failure(
                                request,
                                "compatibility_report",
                                "text_local_recovery",
                            )
                        })?;
                    self.retain_diagnostic_with_owner(
                        request,
                        ResourcePhase::SharedSemanticResolution,
                        "text_local_resource_recovery",
                        diagnostic,
                        DiagnosticOwnership {
                            local_owner: Some(text.source.id()),
                            source_order: Some(text.source_order),
                            source_position: Some(0),
                        },
                    )?;
                    self.charge_local_omission(
                        request,
                        Some(text.source.id()),
                        text.source.pointer(),
                        "text_local_omission",
                    )?;
                    text.render = false;
                    continue;
                }
                for codepoint in &text.text.missing_codepoints {
                    let diagnostic = official_missing_glyph_diagnostic(
                        request.format(),
                        text.source.id(),
                        text.source.pointer(),
                        *codepoint,
                    )
                    .map_err(|_| {
                        self.memory_failure(
                            request,
                            "compatibility_report",
                            "official_missing_glyph",
                        )
                    })?;
                    self.retain_diagnostic_with_owner(
                        request,
                        ResourcePhase::SharedSemanticResolution,
                        "official_missing_glyph",
                        diagnostic,
                        DiagnosticOwnership {
                            local_owner: Some(text.source.id()),
                            source_order: Some(text.source_order),
                            source_position: Some(u64::MAX),
                        },
                    )?;
                }
                self.transact(request, context, delta)?;
                for glyph_id in text
                    .text
                    .lines
                    .iter()
                    .flat_map(|line| line.glyphs.iter().map(|glyph| glyph.id))
                {
                    if self.excalifont_glyphs.contains(&glyph_id) {
                        continue;
                    }
                    self.transact(
                        request,
                        ResourceContext::new(
                            ResourcePhase::SharedSemanticResolution,
                            "excalifont_glyph_selection",
                            text.source.pointer(),
                        )
                        .with_element_id(Some(text.source.id()))
                        .with_local_owner(Some(text.source.id()))
                        .with_record(ResourceRecordKind::BuiltInGlyphOrRangeState),
                        ResourceDelta {
                            working_bytes: 64,
                            ..ResourceDelta::default()
                        },
                    )?;
                    self.excalifont_glyphs.try_reserve(1).map_err(|_| {
                        self.memory_failure(request, "built_in_selection", "excalifont_glyph")
                    })?;
                    self.excalifont_glyphs.push(glyph_id);
                }
                let derived_bytes = glyphs.saturating_mul(8).saturating_add(
                    u64::try_from(text.text.missing_codepoints.len())
                        .unwrap_or(u64::MAX)
                        .saturating_mul(4),
                );
                if derived_bytes != 0 {
                    let previous = self.excalifont_request_derived_bytes;
                    let next = previous.saturating_add(derived_bytes);
                    let working_bytes = logical_aligned_bytes(next)
                        .saturating_sub(logical_aligned_bytes(previous))
                        .saturating_add(u64::from(previous == 0) * 256);
                    self.transact(
                        request,
                        ResourceContext::new(
                            ResourcePhase::SharedSemanticResolution,
                            "excalifont_request_derived_data",
                            text.source.pointer(),
                        )
                        .with_element_id(Some(text.source.id()))
                        .with_local_owner(Some(text.source.id()))
                        .with_record(ResourceRecordKind::RequestDerivedBuiltInBlob),
                        ResourceDelta {
                            working_bytes,
                            ..ResourceDelta::default()
                        },
                    )?;
                    self.excalifont_request_derived_bytes = next;
                }
                semantic_working.try_reserve(1).map_err(|_| {
                    self.memory_failure(request, "semantic_geometry", "text_working_record")
                })?;
                semantic_working.push(SemanticWorkingRecord {
                    source_order: text.source_order,
                    event: "resolved_text_layout_and_glyph_geometry",
                    element_id: Some(try_owned(text.source.id()).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "text_identity")
                    })?),
                    local_owner: Some(try_owned(text.source.id()).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "text_owner")
                    })?),
                    json_pointer: try_owned(text.source.pointer()).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "text_pointer")
                    })?,
                    bytes: working_bytes,
                });
                continue;
            }
            if let ResolvedElement::Frame(frame) = element {
                let outline_segments = frame.outline_path.as_ref().map_or(0, |path| {
                    u64::try_from(path.operations.len()).unwrap_or(u64::MAX)
                });
                let clip_segments = frame.frame_clips.iter().fold(0_u64, |total, clip| {
                    total.saturating_add(
                        u64::try_from(clip.path.operations.len()).unwrap_or(u64::MAX),
                    )
                });
                let label_segments = frame.name.as_ref().map_or(0, |label| {
                    u64::try_from(label.path.operations.len()).unwrap_or(u64::MAX)
                });
                let text_scalars = frame.name.as_ref().map_or(0, |label| {
                    u64::try_from(label.text.chars().count()).unwrap_or(u64::MAX)
                });
                let geometry_segments = outline_segments
                    .saturating_add(clip_segments)
                    .saturating_add(label_segments);
                let total_work_units = geometry_segments
                    .saturating_mul(32)
                    .saturating_add(text_scalars.saturating_mul(32));
                let working_bytes = 128_u64
                    .saturating_add(geometry_segments.saturating_mul(64))
                    .saturating_add(text_scalars.saturating_mul(4));
                let delta = ResourceDelta {
                    geometry_segments,
                    text_scalars,
                    working_bytes,
                    total_work_units,
                    ..ResourceDelta::default()
                };
                let context = ResourceContext::new(
                    ResourcePhase::SharedSemanticResolution,
                    "frame_geometry_and_name",
                    frame.source.pointer(),
                )
                .with_element_id(Some(frame.source.id()))
                .with_local_owner(Some(frame.source.id()));
                self.transact(request, context, delta)?;
                semantic_working.try_reserve(1).map_err(|_| {
                    self.memory_failure(request, "semantic_geometry", "frame_working_record")
                })?;
                semantic_working.push(SemanticWorkingRecord {
                    source_order: frame.source_order,
                    event: "resolved_frame_geometry",
                    element_id: Some(try_owned(frame.source.id()).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "frame_identity")
                    })?),
                    local_owner: Some(try_owned(frame.source.id()).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "frame_owner")
                    })?),
                    json_pointer: try_owned(frame.source.pointer()).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "frame_pointer")
                    })?,
                    bytes: working_bytes,
                });
                continue;
            }
            let ResolvedElement::Fallback(fallback) = element else {
                continue;
            };
            let clip_segments = fallback.frame_clips.iter().fold(0_u64, |total, clip| {
                total.saturating_add(u64::try_from(clip.path.operations.len()).unwrap_or(u64::MAX))
            });
            let geometry_segments = fallback
                .geometry_command_count()
                .saturating_add(clip_segments);
            let glyphs = fallback.label.as_ref().map_or(0, |label| label.glyph_count);
            let working_bytes = 128_u64.saturating_add(geometry_segments.saturating_mul(64));
            let total_work_units = geometry_segments
                .saturating_mul(32)
                .saturating_add(glyphs.saturating_mul(32));
            let delta = ResourceDelta {
                geometry_segments,
                working_bytes,
                total_work_units,
                ..ResourceDelta::default()
            };
            let context = ResourceContext::new(
                ResourcePhase::SharedSemanticResolution,
                "compatibility_fallback_geometry",
                &fallback.source.pointer,
            )
            .with_element_id(fallback.source.id.as_deref())
            .with_local_owner(fallback.source.id.as_deref());
            if self.meter.would_cross_fatal(delta) {
                self.transact(request, context, delta)?;
                return Err(self.internal_failure(
                    request.format(),
                    "fatal fallback resource preflight committed unexpectedly",
                    "/elements",
                ));
            }
            let local_crossing = if geometry_segments
                > u64::from(
                    request
                        .resource_limits()
                        .max_geometry_segments_per_element(),
                ) {
                Some(ElementLocalCrossing {
                    code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
                    field: "max_geometry_segments_per_element",
                    configured: ResourceValue::Integer(u64::from(
                        request
                            .resource_limits()
                            .max_geometry_segments_per_element(),
                    )),
                    value_kind: ValueKind::Count,
                    attempted: ResourceValue::Integer(geometry_segments),
                    pointer_field: None,
                    message:
                        "compatibility fallback omitted because its geometry exceeds its limit",
                })
            } else if total_work_units > request.resource_limits().max_element_work_units() {
                Some(ElementLocalCrossing {
                    code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
                    field: "max_element_work_units",
                    configured: ResourceValue::Integer(
                        request.resource_limits().max_element_work_units(),
                    ),
                    value_kind: ValueKind::WorkUnits,
                    attempted: ResourceValue::Integer(total_work_units),
                    pointer_field: None,
                    message:
                        "compatibility fallback omitted because its semantic work exceeds its limit",
                })
            } else {
                None
            };
            if let Some(crossing) = local_crossing {
                let diagnostic = crossing
                    .diagnostic(
                        request.format(),
                        fallback.source.id.as_deref(),
                        &fallback.source.pointer,
                    )
                    .map_err(|_| {
                        self.memory_failure(
                            request,
                            "compatibility_report",
                            "fallback_local_recovery",
                        )
                    })?;
                self.retain_diagnostic_with_owner(
                    request,
                    ResourcePhase::SharedSemanticResolution,
                    "fallback_local_resource_recovery",
                    diagnostic,
                    DiagnosticOwnership {
                        local_owner: fallback.source.id.as_deref(),
                        source_order: Some(fallback.source_order),
                        source_position: Some(0),
                    },
                )?;
                self.charge_local_omission(
                    request,
                    fallback.source.id.as_deref(),
                    &fallback.source.pointer,
                    "fallback_local_omission",
                )?;
                fallback.render = false;
                continue;
            }
            self.retain_fallback_font_diagnostics(request, fallback)?;
            self.transact(request, context, delta)?;
            semantic_working.try_reserve(1).map_err(|_| {
                self.memory_failure(request, "semantic_geometry", "fallback_working_record")
            })?;
            semantic_working.push(SemanticWorkingRecord {
                source_order: fallback.source_order,
                event: "resolved_compatibility_fallback",
                element_id: fallback
                    .source
                    .id
                    .as_deref()
                    .map(try_owned)
                    .transpose()
                    .map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "fallback_identity")
                    })?,
                local_owner: fallback
                    .source
                    .id
                    .as_deref()
                    .map(try_owned)
                    .transpose()
                    .map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "fallback_owner")
                    })?,
                json_pointer: try_owned(&fallback.source.pointer).map_err(|_| {
                    self.memory_failure(request, "semantic_geometry", "fallback_pointer")
                })?,
                bytes: working_bytes,
            });
        }
        resolved.elements.retain(|element| {
            !matches!(element, ResolvedElement::Fallback(fallback) if !fallback.render)
                && !matches!(element, ResolvedElement::Text(text) if !text.render)
        });
        let has_fallback_label = resolved.elements.iter().any(|element| match element {
            ResolvedElement::Frame(frame) => frame.name.is_some(),
            ResolvedElement::Fallback(fallback) => fallback.label.is_some(),
            _ => false,
        });
        let has_official_text = resolved
            .elements
            .iter()
            .any(|element| matches!(element, ResolvedElement::Text(_)));
        if !has_fallback_label {
            resolved.fallback_font_woff2 = None;
            resolved.fallback_font_sfnt = None;
        }
        if !has_official_text {
            resolved.official_fonts.clear();
        }
        resolved.mandatory_manifest = if has_fallback_label || has_official_text {
            Some(
                text::mandatory_notice_manifest(has_fallback_label, &resolved.official_fonts)
                    .map_err(|error| match error {
                        text::LabelError::Memory => self.memory_failure(
                            request,
                            "artifact_manifest",
                            "font_notice_manifest",
                        ),
                        text::LabelError::InvalidFont => self.internal_failure(
                            request.format(),
                            "the mandatory font notice could not be resolved",
                            "/elements",
                        ),
                    })?,
            )
        } else {
            None
        };
        semantic_working.sort_unstable_by_key(|record| record.source_order);
        Ok(())
    }

    fn preflight_official_text(
        &mut self,
        source: &NormalizedElement,
        request: &RenderRequest,
        semantic_working: &mut Vec<SemanticWorkingRecord>,
    ) -> Result<bool, RenderError> {
        let NormalizedElementKind::Text(text) = &source.kind else {
            return Err(self.internal_failure(
                request.format(),
                "official text preflight received a non-text element",
                "/elements",
            ));
        };
        let text_scalars = u64::try_from(text.text.chars().count()).unwrap_or(u64::MAX);
        let source_bytes = u64::try_from(text.text.len()).unwrap_or(u64::MAX);
        let prospective_glyphs = u64::try_from(
            text.text
                .chars()
                .filter(|character| *character != '\n')
                .count(),
        )
        .unwrap_or(u64::MAX);
        let stored_line_steps = u64::try_from(text.text.split('\n').count()).unwrap_or(u64::MAX);
        let prepaid_work = text_scalars
            .saturating_mul(32)
            .saturating_add(prospective_glyphs.saturating_mul(32))
            .saturating_add(stored_line_steps.saturating_mul(32));
        let working_bytes = 256_u64.saturating_add(logical_aligned_bytes(source_bytes));
        let delta = ResourceDelta {
            text_scalars,
            working_bytes,
            total_work_units: prepaid_work,
            ..ResourceDelta::default()
        };
        let context = ResourceContext::new(
            ResourcePhase::SharedSemanticResolution,
            "text_scalar_and_run_preflight",
            source.source.pointer(),
        )
        .with_element_id(Some(source.source.id()))
        .with_local_owner(Some(source.source.id()));
        if self.meter.would_cross_fatal(delta) {
            self.transact(request, context, delta)?;
            return Err(self.internal_failure(
                request.format(),
                "fatal text scalar preflight committed unexpectedly",
                "/elements",
            ));
        }
        let crossing = if text_scalars
            > u64::from(request.resource_limits().max_text_scalars_per_element())
        {
            Some(ElementLocalCrossing {
                code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
                field: "max_text_scalars_per_element",
                configured: ResourceValue::Integer(u64::from(
                    request.resource_limits().max_text_scalars_per_element(),
                )),
                value_kind: ValueKind::Count,
                attempted: ResourceValue::Integer(text_scalars),
                pointer_field: None,
                message: "text was omitted because it exceeds its scalar limit",
            })
        } else if prepaid_work > request.resource_limits().max_element_work_units() {
            Some(ElementLocalCrossing {
                code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
                field: "max_element_work_units",
                configured: ResourceValue::Integer(
                    request.resource_limits().max_element_work_units(),
                ),
                value_kind: ValueKind::WorkUnits,
                attempted: ResourceValue::Integer(prepaid_work),
                pointer_field: None,
                message: "text was omitted before shaping because scalar work exceeds its limit",
            })
        } else {
            None
        };
        if let Some(crossing) = crossing {
            let diagnostic = crossing
                .diagnostic(
                    request.format(),
                    Some(source.source.id()),
                    source.source.pointer(),
                )
                .map_err(|_| {
                    self.memory_failure(request, "compatibility_report", "text_preflight_recovery")
                })?;
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::SharedSemanticResolution,
                "text_preflight_local_recovery",
                diagnostic,
                DiagnosticOwnership {
                    local_owner: Some(source.source.id()),
                    source_order: Some(source.source_order),
                    source_position: Some(0),
                },
            )?;
            self.charge_local_omission(
                request,
                Some(source.source.id()),
                source.source.pointer(),
                "text_preflight_local_omission",
            )?;
            return Ok(false);
        }
        self.transact(request, context, delta)?;
        semantic_working.try_reserve(1).map_err(|_| {
            self.memory_failure(request, "semantic_geometry", "text_run_working_record")
        })?;
        semantic_working.push(SemanticWorkingRecord {
            source_order: source.source_order,
            event: "resolved_text_run",
            element_id: Some(try_owned(source.source.id()).map_err(|_| {
                self.memory_failure(request, "semantic_geometry", "text_run_identity")
            })?),
            local_owner: Some(try_owned(source.source.id()).map_err(|_| {
                self.memory_failure(request, "semantic_geometry", "text_run_owner")
            })?),
            json_pointer: try_owned(source.source.pointer()).map_err(|_| {
                self.memory_failure(request, "semantic_geometry", "text_run_pointer")
            })?,
            bytes: working_bytes,
        });
        if !self.excalifont_selected {
            self.transact(
                request,
                ResourceContext::new(
                    ResourcePhase::SharedSemanticResolution,
                    "excalifont_builtin_selection",
                    source.source.pointer(),
                )
                .with_element_id(Some(source.source.id()))
                .with_local_owner(Some(source.source.id()))
                .with_record(ResourceRecordKind::BuiltInSelectionState),
                ResourceDelta {
                    working_bytes: 256,
                    ..ResourceDelta::default()
                },
            )?;
            self.excalifont_selected = true;
        }
        if !self.excalifont_range_selected {
            self.transact(
                request,
                ResourceContext::new(
                    ResourcePhase::SharedSemanticResolution,
                    "excalifont_basic_latin_range_selection",
                    source.source.pointer(),
                )
                .with_element_id(Some(source.source.id()))
                .with_local_owner(Some(source.source.id()))
                .with_record(ResourceRecordKind::BuiltInGlyphOrRangeState),
                ResourceDelta {
                    working_bytes: 64,
                    ..ResourceDelta::default()
                },
            )?;
            self.excalifont_range_selected = true;
        }
        if text.font_family == 2 && !text.text.is_empty() {
            let diagnostic = fallback_font_diagnostic(
                DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED,
                DiagnosticSeverity::Info,
                request.format(),
                Some(source.source.id()),
                source.source.pointer(),
                None,
            )
            .map_err(|_| {
                self.memory_failure(request, "compatibility_report", "text_font_substitution")
            })?;
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::SharedSemanticResolution,
                "text_font_substitution",
                diagnostic,
                DiagnosticOwnership {
                    local_owner: Some(source.source.id()),
                    source_order: Some(source.source_order),
                    source_position: Some(0),
                },
            )?;
        }
        Ok(true)
    }

    fn retain_fallback_font_diagnostics(
        &mut self,
        request: &RenderRequest,
        fallback: &ResolvedFallback,
    ) -> Result<(), RenderError> {
        let Some(label) = &fallback.label else {
            return Ok(());
        };
        let substitution = fallback_font_diagnostic(
            DiagnosticCode::FONT_LIBERATION_SANS_SUBSTITUTED,
            DiagnosticSeverity::Info,
            request.format(),
            fallback.source.id.as_deref(),
            &fallback.source.pointer,
            None,
        )
        .map_err(|_| {
            self.memory_failure(
                request,
                "compatibility_report",
                "fallback_font_substitution",
            )
        })?;
        self.retain_diagnostic_with_owner(
            request,
            ResourcePhase::SharedSemanticResolution,
            "fallback_font_substitution",
            substitution,
            DiagnosticOwnership {
                local_owner: fallback.source.id.as_deref(),
                source_order: Some(fallback.source_order),
                source_position: Some(u64::MAX - 1),
            },
        )?;
        for codepoint in &label.missing_codepoints {
            let diagnostic = fallback_font_diagnostic(
                DiagnosticCode::FONT_MISSING_GLYPH,
                DiagnosticSeverity::Warning,
                request.format(),
                fallback.source.id.as_deref(),
                &fallback.source.pointer,
                Some(*codepoint),
            )
            .map_err(|_| {
                self.memory_failure(request, "compatibility_report", "fallback_missing_glyph")
            })?;
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::SharedSemanticResolution,
                "fallback_missing_glyph",
                diagnostic,
                DiagnosticOwnership {
                    local_owner: fallback.source.id.as_deref(),
                    source_order: Some(fallback.source_order),
                    source_position: Some(u64::MAX),
                },
            )?;
        }
        Ok(())
    }

    fn preflight_fallback_text(
        &mut self,
        fallbacks: &mut [crate::scene_surface::SurfaceFallback],
        request: &RenderRequest,
        semantic_working: &mut Vec<SemanticWorkingRecord>,
    ) -> Result<(), RenderError> {
        let mut index = 0_usize;
        while index < fallbacks.len() {
            let fallback = &fallbacks[index];
            let text_scalars = u64::try_from(fallback.label.chars().count()).unwrap_or(u64::MAX);
            let working_bytes = 256_u64.saturating_add(logical_aligned_bytes(
                u64::try_from(fallback.label.len()).unwrap_or(u64::MAX),
            ));
            let work_units = text_scalars.saturating_mul(32);
            let base_geometry_segments = if fallback.width >= 2.0 && fallback.height >= 2.0 {
                9
            } else {
                5
            };
            let geometry_preflight = ResourceDelta {
                geometry_segments: base_geometry_segments,
                ..ResourceDelta::default()
            };
            let geometry_context = ResourceContext::new(
                ResourcePhase::SharedSemanticResolution,
                "compatibility_fallback_geometry_preflight",
                &fallback.json_pointer,
            )
            .with_element_id(fallback.element_id.as_deref())
            .with_local_owner(fallback.element_id.as_deref());
            if self.meter.would_cross_fatal(geometry_preflight) {
                self.transact(request, geometry_context, geometry_preflight)?;
                return Err(self.internal_failure(
                    request.format(),
                    "fatal fallback geometry preflight committed unexpectedly",
                    "/elements",
                ));
            }
            let delta = ResourceDelta {
                text_scalars,
                working_bytes,
                total_work_units: work_units,
                ..ResourceDelta::default()
            };
            let context = ResourceContext::new(
                ResourcePhase::SharedSemanticResolution,
                "compatibility_fallback_text",
                &fallback.json_pointer,
            )
            .with_element_id(fallback.element_id.as_deref())
            .with_local_owner(fallback.element_id.as_deref());
            if self.meter.would_cross_fatal(delta) {
                self.transact(request, context, delta)?;
                return Err(self.internal_failure(
                    request.format(),
                    "fatal fallback text preflight committed unexpectedly",
                    "/elements",
                ));
            }
            if text_scalars > u64::from(request.resource_limits().max_text_scalars_per_element()) {
                let diagnostic = ElementLocalCrossing {
                    code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
                    field: "max_text_scalars_per_element",
                    configured: ResourceValue::Integer(u64::from(
                        request.resource_limits().max_text_scalars_per_element(),
                    )),
                    value_kind: ValueKind::Count,
                    attempted: ResourceValue::Integer(text_scalars),
                    pointer_field: None,
                    message:
                        "compatibility fallback omitted because its label exceeds the text limit",
                }
                .diagnostic(
                    request.format(),
                    fallback.element_id.as_deref(),
                    &fallback.json_pointer,
                )
                .map_err(|_| {
                    self.memory_failure(request, "compatibility_report", "fallback_text_recovery")
                })?;
                self.retain_diagnostic_with_owner(
                    request,
                    ResourcePhase::SharedSemanticResolution,
                    "fallback_text_local_resource_recovery",
                    diagnostic,
                    DiagnosticOwnership {
                        local_owner: fallback.element_id.as_deref(),
                        source_order: Some(fallback.source_order),
                        source_position: Some(0),
                    },
                )?;
                self.charge_local_omission(
                    request,
                    fallback.element_id.as_deref(),
                    &fallback.json_pointer,
                    "fallback_text_local_omission",
                )?;
                fallbacks[index].paint = false;
                index = index.saturating_add(1);
                continue;
            }
            self.transact(request, context, delta)?;
            semantic_working.try_reserve(1).map_err(|_| {
                self.memory_failure(request, "semantic_text", "fallback_text_working_record")
            })?;
            semantic_working.push(SemanticWorkingRecord {
                source_order: fallback.source_order,
                event: "resolved_compatibility_fallback_text",
                element_id: fallback
                    .element_id
                    .as_deref()
                    .map(try_owned)
                    .transpose()
                    .map_err(|_| {
                        self.memory_failure(request, "semantic_text", "fallback_text_identity")
                    })?,
                local_owner: fallback
                    .element_id
                    .as_deref()
                    .map(try_owned)
                    .transpose()
                    .map_err(|_| {
                        self.memory_failure(request, "semantic_text", "fallback_text_owner")
                    })?,
                json_pointer: try_owned(&fallback.json_pointer).map_err(|_| {
                    self.memory_failure(request, "semantic_text", "fallback_text_pointer")
                })?,
                bytes: working_bytes,
            });
            index = index.saturating_add(1);
        }
        Ok(())
    }

    fn account_compatibility_omissions(
        &mut self,
        request: &RenderRequest,
    ) -> Result<(), RenderError> {
        let omissions = (|| -> Result<_, std::collections::TryReserveError> {
            let mut omissions = Vec::new();
            for diagnostic in &self.report.diagnostics {
                if !matches!(
                    diagnostic.code(),
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_ELEMENT_TYPE
                        | DiagnosticCode::COMPATIBILITY_MALFORMED_ELEMENT
                        | DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
                ) || !matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::Compatibility {
                        recovery: Recovery::Omitted,
                        ..
                    })
                ) {
                    continue;
                }
                omissions.try_reserve(1)?;
                omissions.push((
                    diagnostic.element_id().map(try_owned).transpose()?,
                    try_owned(diagnostic.json_pointer())?,
                ));
            }
            Ok(omissions)
        })()
        .map_err(|_| self.memory_failure(request, "semantic_omission", "omission_list"))?;
        for (element_id, pointer) in &omissions {
            self.charge_work(
                request,
                ResourceContext::new(
                    ResourcePhase::SharedSemanticResolution,
                    "compatibility_omission",
                    pointer,
                )
                .with_element_id(element_id.as_deref())
                .with_local_owner(element_id.as_deref()),
                8,
            )?;
        }
        Ok(())
    }

    fn charge_local_omission(
        &mut self,
        request: &RenderRequest,
        element_id: Option<&str>,
        json_pointer: &str,
        event: &'static str,
    ) -> Result<(), RenderError> {
        self.charge_work(
            request,
            ResourceContext::new(ResourcePhase::SharedSemanticResolution, event, json_pointer)
                .with_element_id(element_id)
                .with_local_owner(element_id),
            8,
        )
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
        self.retain_diagnostic_with_owner(
            request,
            phase,
            event,
            diagnostic,
            DiagnosticOwnership::default(),
        )
    }

    fn retain_diagnostic_with_owner(
        &mut self,
        request: &RenderRequest,
        phase: ResourcePhase,
        event: &'static str,
        mut diagnostic: CompatibilityDiagnostic,
        ownership: DiagnosticOwnership<'_>,
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
                .with_local_owner(ownership.local_owner.or(diagnostic.element_id.as_deref()))
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
            owner_index: ownership
                .source_order
                .or_else(|| {
                    element_owner_index(&diagnostic.json_pointer)
                        .map(|index| self.element_source_order_base.saturating_add(index))
                })
                .unwrap_or(u64::MAX),
            source_position: ownership.source_position.unwrap_or(u64::MAX),
            retention_ordinal: u64::try_from(self.report.diagnostics.len()).unwrap_or(u64::MAX),
        };
        self.report.diagnostics.push(diagnostic);
        Ok(())
    }

    fn take_report(&mut self) -> CompatibilityReport {
        self.report.diagnostics.sort_unstable_by(|left, right| {
            left.order
                .owner_index
                .cmp(&right.order.owner_index)
                .then_with(|| left.order.source_position.cmp(&right.order.source_position))
                .then_with(|| left.json_pointer.cmp(&right.json_pointer))
                .then_with(|| left.code.as_str().cmp(right.code.as_str()))
                .then_with(|| {
                    left.order
                        .retention_ordinal
                        .cmp(&right.order.retention_ordinal)
                })
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
                    record.event,
                    &record.json_pointer,
                )
                .with_element_id(record.element_id.as_deref())
                .with_local_owner(record.local_owner.as_deref()),
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

    fn unbounded_geometry_failure(&mut self, format: OutputFormat) -> RenderError {
        RenderError {
            category: RenderErrorCategory::Input,
            code: ErrorCode::SCENE_UNBOUNDED_GEOMETRY,
            message: Box::from("finite whole-scene bounds could not be established"),
            format,
            element_id: None,
            json_pointer: Box::from(""),
            details: None,
            report: self.take_report(),
        }
    }
}

struct ResolvedScene {
    logical_width: f64,
    logical_height: f64,
    include_background: bool,
    background_color: CanonicalColor,
    fallback_colors: FallbackColors,
    elements: Vec<ResolvedElement>,
    fallback_font_woff2: Option<Vec<u8>>,
    fallback_font_sfnt: Option<Vec<u8>>,
    official_fonts: Vec<ResolvedFont>,
    mandatory_manifest: Option<String>,
    _relationships: crate::relationships::ResolvedRelationshipGraph,
}

struct ResolvedFont {
    family: &'static str,
    pdf_name: &'static str,
    identity: &'static str,
    woff2: Vec<u8>,
    sfnt: Vec<u8>,
}

#[derive(Clone, Copy)]
struct EffectiveFrameRendering {
    enabled: bool,
    clip: bool,
    name: bool,
    outline: bool,
}

enum GeneratedGeometry {
    Rough(rough::Drawing),
    Freedraw(GeneratedFreedraw),
}

struct GeneratedFreedraw {
    path: freedraw::Path,
    background_fill: Option<rough::Fill>,
}

impl GeneratedGeometry {
    fn operation_count(&self) -> usize {
        match self {
            Self::Rough(drawing) => drawing.operation_count(),
            Self::Freedraw(freedraw) => {
                freedraw.path.operation_count()
                    + freedraw
                        .background_fill
                        .as_ref()
                        .map_or(0, |fill| fill.path.operations.len())
            }
        }
    }

    fn bounds(&self, source: &NormalizedElement) -> (f64, f64, f64, f64) {
        match self {
            Self::Rough(drawing) => local_outline_bounds(source, Some(drawing)),
            Self::Freedraw(_) => source_layout_bounds(source),
        }
    }
}

fn source_layout_bounds(source: &NormalizedElement) -> (f64, f64, f64, f64) {
    match &source.kind {
        NormalizedElementKind::Freedraw(freedraw) if !freedraw.points.is_empty() => {
            freedraw.points.iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(min_x, min_y, max_x, max_y), point| {
                    (
                        min_x.min(point[0]),
                        min_y.min(point[1]),
                        max_x.max(point[0]),
                        max_y.max(point[1]),
                    )
                },
            )
        }
        _ => (0.0, 0.0, source.width, source.height),
    }
}

fn relationship_frame_target(
    source_index: usize,
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> Option<usize> {
    let node_index = relationships
        .nodes
        .iter()
        .position(|node| node.source_index == source_index)?;
    relationships.edges.iter().find_map(|edge| {
        if edge.source != node_index || edge.kind != crate::relationships::RelationshipKind::Frame {
            return None;
        }
        let crate::relationships::RelationshipTarget::Element(target) = edge.target else {
            return None;
        };
        relationships
            .nodes
            .get(target)
            .map(|node| node.source_index)
    })
}

fn resolve_frame_clips(
    source_index: usize,
    frames: &[SemanticFrameSource],
    relationships: &crate::relationships::ResolvedRelationshipGraph,
    rendering: EffectiveFrameRendering,
    min_x: f64,
    min_y: f64,
    padding: f64,
) -> Result<Vec<ResolvedClip>, ResolveSceneError> {
    let mut clips = Vec::new();
    if !rendering.enabled || !rendering.clip {
        return Ok(clips);
    }
    let Some(parent_index) = relationship_frame_target(source_index, relationships) else {
        return Ok(clips);
    };
    let Some(frame) = frames
        .iter()
        .find(|frame| frame.source_index == parent_index)
    else {
        return Ok(clips);
    };
    clips.try_reserve_exact(1)?;
    clips.push(ResolvedClip {
        path: resolved_frame_outline(frame.width, frame.height)?,
        width: frame.width,
        height: frame.height,
        transform: ResolvedTransform {
            translation_x: frame.x - min_x + padding,
            translation_y: frame.y - min_y + padding,
            angle_radians: frame.angle,
            center_x: frame.width / 2.0,
            center_y: frame.height / 2.0,
        },
    });
    Ok(clips)
}

fn resolve_frame_opacity(
    source_index: usize,
    frames: &[SemanticFrameSource],
    relationships: &crate::relationships::ResolvedRelationshipGraph,
    _rendering: EffectiveFrameRendering,
) -> f64 {
    relationship_frame_target(source_index, relationships)
        .and_then(|parent_index| {
            frames
                .iter()
                .find(|frame| frame.source_index == parent_index)
        })
        .map_or(1.0, |frame| frame.opacity)
}

enum ResolveSceneError {
    Memory,
    InvalidFallbackFont,
    InvalidColor,
    UnboundedGeometry,
}

impl From<std::collections::TryReserveError> for ResolveSceneError {
    fn from(_: std::collections::TryReserveError) -> Self {
        Self::Memory
    }
}

impl ResolvedScene {
    fn empty(
        relationships: crate::relationships::ResolvedRelationshipGraph,
        background_color: &str,
        request: &RenderRequest,
    ) -> Result<Self, ResolveSceneError> {
        let logical_size = request.padding() * 2.0;
        if !logical_size.is_finite() {
            return Err(ResolveSceneError::UnboundedGeometry);
        }
        Ok(Self {
            logical_width: logical_size.max(1.0),
            logical_height: logical_size.max(1.0),
            include_background: request.include_background(),
            background_color: color::parse_css(background_color)
                .ok_or(ResolveSceneError::InvalidColor)?
                .for_mode(request.color_mode()),
            fallback_colors: FallbackColors::for_mode(request.color_mode()),
            elements: Vec::new(),
            fallback_font_woff2: None,
            fallback_font_sfnt: None,
            official_fonts: Vec::new(),
            mandatory_manifest: None,
            _relationships: relationships,
        })
    }

    fn elements(
        outlines: Vec<(NormalizedElement, Option<GeneratedGeometry>)>,
        fallbacks: Vec<crate::scene_surface::SurfaceFallback>,
        relationships: crate::relationships::ResolvedRelationshipGraph,
        frame_rendering: EffectiveFrameRendering,
        background_color: &str,
        request: &RenderRequest,
    ) -> Result<Option<Self>, ResolveSceneError> {
        if outlines.is_empty() && fallbacks.is_empty() {
            return Ok(None);
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (source, geometry) in &outlines {
            if source.stroke_width < 0.0
                || source.roughness < 0.0
                || !(0.0..=100.0).contains(&source.opacity)
                || source.seed == 0
            {
                return Ok(None);
            }
            let is_frame = matches!(
                source.kind,
                NormalizedElementKind::Frame(_) | NormalizedElementKind::MagicFrame(_)
            );
            if is_frame && !frame_rendering.enabled {
                continue;
            }
            if frame_rendering.enabled
                && frame_rendering.clip
                && relationship_frame_target(source.source.source_index(), &relationships).is_some()
            {
                continue;
            }
            let (local_min_x, mut local_min_y, local_max_x, local_max_y) =
                geometry.as_ref().map_or_else(
                    || source_layout_bounds(source),
                    |geometry| geometry.bounds(source),
                );
            if is_frame && frame_rendering.name {
                local_min_y = local_min_y.min(-24.0);
            }
            let (center_x, center_y) = if is_frame {
                (
                    source.x + source.width / 2.0,
                    source.y + source.height / 2.0,
                )
            } else {
                (
                    source.x + (local_min_x + local_max_x) / 2.0,
                    source.y + (local_min_y + local_max_y) / 2.0,
                )
            };
            let sin = source.angle.sin();
            let cos = source.angle.cos();
            if let NormalizedElementKind::Freedraw(freedraw) = &source.kind {
                if !freedraw.points.is_empty() {
                    for point in &freedraw.points {
                        let point_x = source.x + point[0];
                        let point_y = source.y + point[1];
                        let delta_x = point_x - center_x;
                        let delta_y = point_y - center_y;
                        let rotated_x = center_x + delta_x * cos - delta_y * sin;
                        let rotated_y = center_y + delta_x * sin + delta_y * cos;
                        min_x = min_x.min(rotated_x);
                        min_y = min_y.min(rotated_y);
                        max_x = max_x.max(rotated_x);
                        max_y = max_y.max(rotated_y);
                    }
                    continue;
                }
            }
            for (corner_x, corner_y) in [
                (source.x + local_min_x, source.y + local_min_y),
                (source.x + local_max_x, source.y + local_min_y),
                (source.x + local_max_x, source.y + local_max_y),
                (source.x + local_min_x, source.y + local_max_y),
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
        for fallback in &fallbacks {
            let is_clipped_child = frame_rendering.enabled
                && frame_rendering.clip
                && fallback.element_id.as_deref().is_some_and(|id| {
                    relationships
                        .nodes
                        .iter()
                        .find(|node| node.id == id)
                        .and_then(|node| {
                            relationship_frame_target(node.source_index, &relationships)
                        })
                        .is_some()
                });
            if is_clipped_child {
                continue;
            }
            include_transformed_bounds(
                fallback.x,
                fallback.y,
                fallback.width,
                fallback.height,
                fallback.angle,
                &mut min_x,
                &mut min_y,
                &mut max_x,
                &mut max_y,
            );
        }
        if min_x == f64::INFINITY
            && min_y == f64::INFINITY
            && max_x == f64::NEG_INFINITY
            && max_y == f64::NEG_INFINITY
        {
            return Self::empty(relationships, background_color, request).map(Some);
        }
        let logical_width = max_x - min_x + request.padding() * 2.0;
        let logical_height = max_y - min_y + request.padding() * 2.0;
        if !logical_width.is_finite() || !logical_height.is_finite() {
            return Err(ResolveSceneError::UnboundedGeometry);
        }

        let mut elements = Vec::new();
        elements.try_reserve_exact(outlines.len().saturating_add(fallbacks.len()))?;
        let background_color = color::parse_css(background_color)
            .ok_or(ResolveSceneError::InvalidColor)?
            .for_mode(request.color_mode());
        let mut semantic_frames = Vec::new();
        semantic_frames.try_reserve(outlines.len())?;
        for (source, _) in &outlines {
            if matches!(
                source.kind,
                NormalizedElementKind::Frame(_) | NormalizedElementKind::MagicFrame(_)
            ) {
                semantic_frames.push(SemanticFrameSource {
                    source_index: source.source.source_index(),
                    x: source.x,
                    y: source.y,
                    width: source.width,
                    height: source.height,
                    angle: source.angle,
                    opacity: source.opacity / 100.0,
                });
            }
        }
        let mut fallback_frame_clips = Vec::new();
        if frame_rendering.enabled && frame_rendering.clip {
            fallback_frame_clips.try_reserve(fallbacks.len())?;
            for fallback in &fallbacks {
                let Some(id) = fallback.element_id.as_deref().filter(|_| fallback.is_frame) else {
                    continue;
                };
                fallback_frame_clips.push(FallbackFrameClipSource {
                    id: try_owned(id)?,
                    x: fallback.x,
                    y: fallback.y,
                    width: fallback.width,
                    height: fallback.height,
                    angle: fallback.angle,
                });
            }
        }
        let needs_frame_font =
            frame_rendering.enabled && frame_rendering.name && !semantic_frames.is_empty();
        let fallback_font = if needs_frame_font || fallbacks.iter().any(|fallback| fallback.paint) {
            Some(text::FallbackFont::load().map_err(|error| match error {
                text::LabelError::Memory => ResolveSceneError::Memory,
                text::LabelError::InvalidFont => ResolveSceneError::InvalidFallbackFont,
            })?)
        } else {
            None
        };
        let mut official_fonts = Vec::<text::OfficialFont>::new();
        let mut has_label = false;
        for (source, geometry) in outlines {
            let frame_clips = resolve_frame_clips(
                source.source.source_index(),
                &semantic_frames,
                &relationships,
                frame_rendering,
                min_x,
                min_y,
                request.padding(),
            )?;
            let frame_opacity = resolve_frame_opacity(
                source.source.source_index(),
                &semantic_frames,
                &relationships,
                frame_rendering,
            );
            if let NormalizedElementKind::Frame(frame) | NormalizedElementKind::MagicFrame(frame) =
                &source.kind
            {
                if frame_rendering.enabled {
                    let name = if frame_rendering.name {
                        let text = frame.name.as_deref().unwrap_or({
                            if matches!(source.kind, NormalizedElementKind::MagicFrame(_)) {
                                "AI Frame"
                            } else {
                                "Frame"
                            }
                        });
                        let font = fallback_font
                            .as_ref()
                            .ok_or(ResolveSceneError::InvalidFallbackFont)?;
                        font.resolve(text, source.width, 20.0)
                            .map_err(|error| match error {
                                text::LabelError::Memory => ResolveSceneError::Memory,
                                text::LabelError::InvalidFont => {
                                    ResolveSceneError::InvalidFallbackFont
                                }
                            })?
                    } else {
                        None
                    };
                    has_label |= name.is_some();
                    let outline_path = frame_rendering
                        .outline
                        .then_some(resolved_frame_outline(source.width, source.height))
                        .transpose()?;
                    elements.push(ResolvedElement::Frame(ResolvedFrame {
                        source_order: source.source_order,
                        source: source.source,
                        name,
                        magic: matches!(source.kind, NormalizedElementKind::MagicFrame(_)),
                        outline_path,
                        frame_clips,
                        transform: ResolvedTransform {
                            translation_x: source.x - min_x + request.padding(),
                            translation_y: source.y - min_y + request.padding(),
                            angle_radians: source.angle,
                            center_x: source.width / 2.0,
                            center_y: source.height / 2.0,
                        },
                        opacity: source.opacity / 100.0 * frame_opacity,
                    }));
                }
                continue;
            }
            if let NormalizedElementKind::Text(text_source) = &source.kind {
                if text_source.text.is_empty() {
                    continue;
                }
                let font_index = if let Some(index) = official_fonts
                    .iter()
                    .position(|font| font.matches_family(text_source.font_family))
                {
                    index
                } else {
                    official_fonts.try_reserve(1)?;
                    official_fonts.push(
                        text::OfficialFont::load(text_source.font_family, &text_source.text)
                            .map_err(|error| match error {
                                text::LabelError::Memory => ResolveSceneError::Memory,
                                text::LabelError::InvalidFont => {
                                    ResolveSceneError::InvalidFallbackFont
                                }
                            })?,
                    );
                    official_fonts.len() - 1
                };
                let mut laid_out = official_fonts[font_index]
                    .layout(
                        &text_source.text,
                        text::TextLayout {
                            width: source.width,
                            font_size: text_source.font_size,
                            line_height: text_source.line_height,
                            text_align: &text_source.text_align,
                        },
                    )
                    .map_err(|error| match error {
                        text::LabelError::Memory => ResolveSceneError::Memory,
                        text::LabelError::InvalidFont => ResolveSceneError::InvalidFallbackFont,
                    })?;
                laid_out.font_index = font_index;
                elements.push(ResolvedElement::Text(ResolvedTextElement {
                    source_order: source.source_order,
                    source: source.source,
                    text: laid_out,
                    frame_clips,
                    transform: ResolvedTransform {
                        translation_x: source.x - min_x + request.padding(),
                        translation_y: source.y - min_y + request.padding(),
                        angle_radians: source.angle,
                        center_x: source.width / 2.0,
                        center_y: source.height / 2.0,
                    },
                    color: color::parse_css(&source.stroke_color)
                        .ok_or(ResolveSceneError::InvalidColor)?
                        .for_mode(request.color_mode()),
                    opacity: source.opacity / 100.0 * frame_opacity,
                    render: true,
                    base_element_work_units: 0,
                }));
                continue;
            }
            let Some(geometry) = geometry else {
                continue;
            };
            let geometry_work_units = u64::try_from(geometry.operation_count())
                .unwrap_or(u64::MAX)
                .saturating_mul(32);
            let (local_min_x, local_min_y, local_max_x, local_max_y) = geometry.bounds(&source);
            let transform = ResolvedTransform {
                translation_x: source.x - min_x + request.padding(),
                translation_y: source.y - min_y + request.padding(),
                angle_radians: source.angle,
                center_x: (local_min_x + local_max_x) / 2.0,
                center_y: (local_min_y + local_max_y) / 2.0,
            };
            let stroke_color = color::parse_css(&source.stroke_color)
                .ok_or(ResolveSceneError::InvalidColor)?
                .for_mode(request.color_mode());
            let base_element_work_units = source.normalization_work_units
                + geometry_work_units
                + 8
                + source.repair_work_units;
            match geometry {
                GeneratedGeometry::Rough(drawing) => {
                    elements.push(ResolvedElement::Outline(ResolvedOutline {
                        source_order: source.source_order,
                        source: source.source,
                        base_element_work_units,
                        drawing,
                        frame_clips,
                        transform,
                        stroke_color,
                        fill_color: (source.background_color != "transparent")
                            .then(|| color::parse_css(&source.background_color))
                            .flatten()
                            .map(|color| color.for_mode(request.color_mode())),
                        stroke_width: if source.stroke_style == "solid" {
                            source.stroke_width
                        } else {
                            source.stroke_width + 0.5
                        },
                        stroke_style: match source.stroke_style.as_str() {
                            "dashed" => StrokeStyle::Dashed,
                            "dotted" => StrokeStyle::Dotted,
                            _ => StrokeStyle::Solid,
                        },
                        source_stroke_width: source.stroke_width,
                        opacity: source.opacity / 100.0 * frame_opacity,
                        background_color,
                    }));
                }
                GeneratedGeometry::Freedraw(freedraw) => {
                    elements.push(ResolvedElement::Freedraw(ResolvedFreedraw {
                        source_order: source.source_order,
                        source: source.source,
                        base_element_work_units,
                        path: freedraw.path,
                        background_fill: freedraw.background_fill,
                        frame_clips,
                        transform,
                        color: stroke_color,
                        background_color: (source.background_color != "transparent")
                            .then(|| color::parse_css(&source.background_color))
                            .flatten()
                            .map(|color| color.for_mode(request.color_mode())),
                        source_stroke_width: source.stroke_width,
                        opacity: source.opacity / 100.0 * frame_opacity,
                    }));
                }
            }
        }
        for fallback in fallbacks {
            if !fallback.paint {
                continue;
            }
            let Some(font) = fallback_font.as_ref() else {
                return Err(ResolveSceneError::InvalidFallbackFont);
            };
            let label = font
                .resolve(&fallback.label, fallback.width, fallback.height)
                .map_err(|error| match error {
                    text::LabelError::Memory => ResolveSceneError::Memory,
                    text::LabelError::InvalidFont => ResolveSceneError::InvalidFallbackFont,
                })?;
            has_label |= label.is_some();
            let source_index = fallback.element_id.as_deref().and_then(|id| {
                relationships
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .map(|node| node.source_index)
            });
            let frame_clips = if let Some(source_index) = source_index {
                resolve_frame_clips(
                    source_index,
                    &semantic_frames,
                    &relationships,
                    frame_rendering,
                    min_x,
                    min_y,
                    request.padding(),
                )?
            } else {
                let frame = fallback.frame_id.as_deref().and_then(|frame_id| {
                    fallback_frame_clips
                        .iter()
                        .find(|frame| frame.id == frame_id)
                });
                let mut clips = Vec::new();
                if let Some(frame) = frame {
                    clips.try_reserve_exact(1)?;
                    clips.push(ResolvedClip {
                        path: resolved_frame_outline(frame.width, frame.height)?,
                        width: frame.width,
                        height: frame.height,
                        transform: ResolvedTransform {
                            translation_x: frame.x - min_x + request.padding(),
                            translation_y: frame.y - min_y + request.padding(),
                            angle_radians: frame.angle,
                            center_x: frame.width / 2.0,
                            center_y: frame.height / 2.0,
                        },
                    });
                }
                clips
            };
            let frame_opacity = source_index.map_or(1.0, |source_index| {
                resolve_frame_opacity(
                    source_index,
                    &semantic_frames,
                    &relationships,
                    frame_rendering,
                )
            });
            elements.push(ResolvedElement::Fallback(ResolvedFallback {
                source_order: fallback.source_order,
                source: FallbackSource {
                    id: fallback.element_id,
                    pointer: fallback.json_pointer,
                },
                label,
                link: fallback.link,
                frame_clips,
                width: fallback.width,
                height: fallback.height,
                transform: ResolvedTransform {
                    translation_x: fallback.x - min_x + request.padding(),
                    translation_y: fallback.y - min_y + request.padding(),
                    angle_radians: fallback.angle,
                    center_x: fallback.width / 2.0,
                    center_y: fallback.height / 2.0,
                },
                opacity: fallback.opacity / 100.0 * frame_opacity,
                render: true,
            }));
        }
        elements.sort_by_key(ResolvedElement::source_order);
        let (fallback_font_woff2, fallback_font_sfnt) = if has_label {
            fallback_font
                .map(text::FallbackFont::into_parts)
                .map_or((None, None), |(woff2, sfnt)| (Some(woff2), Some(sfnt)))
        } else {
            (None, None)
        };
        let has_official_text = elements
            .iter()
            .any(|element| matches!(element, ResolvedElement::Text(_)));
        let mut resolved_fonts = Vec::new();
        resolved_fonts.try_reserve_exact(official_fonts.len())?;
        for font in official_fonts {
            let (family, pdf_name, identity, woff2, sfnt) = font.into_parts();
            resolved_fonts.push(ResolvedFont {
                family,
                pdf_name,
                identity,
                woff2,
                sfnt,
            });
        }
        let mandatory_manifest = if has_label || has_official_text {
            Some(
                text::mandatory_notice_manifest(has_label, &resolved_fonts).map_err(|error| {
                    match error {
                        text::LabelError::Memory => ResolveSceneError::Memory,
                        text::LabelError::InvalidFont => ResolveSceneError::InvalidFallbackFont,
                    }
                })?,
            )
        } else {
            None
        };
        Ok(Some(Self {
            logical_width,
            logical_height,
            include_background: request.include_background(),
            background_color,
            fallback_colors: FallbackColors::for_mode(request.color_mode()),
            elements,
            fallback_font_woff2,
            fallback_font_sfnt,
            official_fonts: resolved_fonts,
            mandatory_manifest,
            _relationships: relationships,
        }))
    }

    fn source_count(&self) -> usize {
        self.elements.len()
    }

    fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
        self.elements.get(index).map(|element| match element {
            ResolvedElement::Outline(outline) => {
                (Some(outline.source.id()), outline.source.pointer())
            }
            ResolvedElement::Freedraw(freedraw) => {
                (Some(freedraw.source.id()), freedraw.source.pointer())
            }
            ResolvedElement::Frame(frame) => (Some(frame.source.id()), frame.source.pointer()),
            ResolvedElement::Text(text) => (Some(text.source.id()), text.source.pointer()),
            ResolvedElement::Fallback(fallback) => (
                fallback.source.id.as_deref(),
                fallback.source.pointer.as_str(),
            ),
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
    Outline(ResolvedOutline),
    Freedraw(ResolvedFreedraw),
    Frame(ResolvedFrame),
    Text(ResolvedTextElement),
    Fallback(ResolvedFallback),
}

impl ResolvedElement {
    fn source_order(&self) -> u64 {
        match self {
            Self::Outline(outline) => outline.source_order,
            Self::Freedraw(freedraw) => freedraw.source_order,
            Self::Frame(frame) => frame.source_order,
            Self::Text(text) => text.source_order,
            Self::Fallback(fallback) => fallback.source_order,
        }
    }
}

struct ResolvedTextElement {
    source_order: u64,
    source: NormalizedElementSource,
    text: text::ResolvedText,
    frame_clips: Vec<ResolvedClip>,
    transform: ResolvedTransform,
    color: CanonicalColor,
    opacity: f64,
    render: bool,
    base_element_work_units: u64,
}

struct ResolvedOutline {
    source_order: u64,
    source: NormalizedElementSource,
    base_element_work_units: u64,
    drawing: rough::Drawing,
    frame_clips: Vec<ResolvedClip>,
    transform: ResolvedTransform,
    stroke_color: CanonicalColor,
    fill_color: Option<CanonicalColor>,
    stroke_width: f64,
    stroke_style: StrokeStyle,
    source_stroke_width: f64,
    opacity: f64,
    background_color: CanonicalColor,
}

struct ResolvedFreedraw {
    source_order: u64,
    source: NormalizedElementSource,
    base_element_work_units: u64,
    path: freedraw::Path,
    background_fill: Option<rough::Fill>,
    frame_clips: Vec<ResolvedClip>,
    transform: ResolvedTransform,
    color: CanonicalColor,
    background_color: Option<CanonicalColor>,
    source_stroke_width: f64,
    opacity: f64,
}

struct ResolvedFrame {
    source_order: u64,
    source: NormalizedElementSource,
    name: Option<text::ResolvedLabel>,
    magic: bool,
    outline_path: Option<rough::Path>,
    frame_clips: Vec<ResolvedClip>,
    transform: ResolvedTransform,
    opacity: f64,
}

fn resolved_frame_outline(width: f64, height: f64) -> Result<rough::Path, ResolveSceneError> {
    let radius = 8.0_f64.min(width / 2.0).min(height / 2.0).max(0.0);
    let kappa = 0.552_284_749_830_793_6;
    let point = |x, y| rough::Point { x, y };
    let mut operations = Vec::new();
    operations.try_reserve_exact(10)?;
    operations.push(rough::PathOperation::Move(point(radius, 0.0)));
    operations.push(rough::PathOperation::Line(point(width - radius, 0.0)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(width - radius + radius * kappa, 0.0),
        control_2: point(width, radius - radius * kappa),
        end: point(width, radius),
    });
    operations.push(rough::PathOperation::Line(point(width, height - radius)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(width, height - radius + radius * kappa),
        control_2: point(width - radius + radius * kappa, height),
        end: point(width - radius, height),
    });
    operations.push(rough::PathOperation::Line(point(radius, height)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(radius - radius * kappa, height),
        control_2: point(0.0, height - radius + radius * kappa),
        end: point(0.0, height - radius),
    });
    operations.push(rough::PathOperation::Line(point(0.0, radius)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(0.0, radius - radius * kappa),
        control_2: point(radius - radius * kappa, 0.0),
        end: point(radius, 0.0),
    });
    Ok(rough::Path { operations })
}

#[derive(Clone, Copy)]
enum StrokeStyle {
    Solid,
    Dashed,
    Dotted,
}

fn decoration_fill_color(
    decoration: &rough::Decoration,
    stroke: CanonicalColor,
    background: CanonicalColor,
) -> CanonicalColor {
    match decoration.fill_paint {
        rough::DecorationFillPaint::Stroke => stroke,
        rough::DecorationFillPaint::Background => background,
    }
}

fn decoration_stroke_style(
    decoration: &rough::Decoration,
    source_stroke_width: f64,
) -> (StrokeStyle, f64) {
    if decoration.dotted {
        (StrokeStyle::Dotted, source_stroke_width - 2.0)
    } else {
        (StrokeStyle::Solid, source_stroke_width)
    }
}

#[derive(Clone, Copy)]
struct FallbackColors {
    fill: CanonicalColor,
    stroke: CanonicalColor,
    label: CanonicalColor,
}

impl FallbackColors {
    fn for_mode(mode: crate::ColorMode) -> Self {
        Self {
            fill: CanonicalColor::from_rgb8(0xf1, 0xf3, 0xf5).for_mode(mode),
            stroke: CanonicalColor::from_rgb8(0x86, 0x8e, 0x96).for_mode(mode),
            label: CanonicalColor::from_rgb8(0x49, 0x50, 0x57).for_mode(mode),
        }
    }
}

struct ResolvedFallback {
    source_order: u64,
    source: FallbackSource,
    label: Option<text::ResolvedLabel>,
    link: Option<String>,
    frame_clips: Vec<ResolvedClip>,
    width: f64,
    height: f64,
    transform: ResolvedTransform,
    opacity: f64,
    render: bool,
}

struct FallbackFrameClipSource {
    id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
}

struct SemanticFrameSource {
    source_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
    opacity: f64,
}

struct ResolvedClip {
    path: rough::Path,
    width: f64,
    height: f64,
    transform: ResolvedTransform,
}

impl ResolvedFallback {
    fn geometry_command_count(&self) -> u64 {
        let visual_commands: u64 = if self.width >= 2.0 && self.height >= 2.0 {
            9
        } else {
            5
        };
        visual_commands.saturating_add(self.label.as_ref().map_or(0, |label| {
            u64::try_from(label.path.operations.len()).unwrap_or(u64::MAX)
        }))
    }
}

struct FallbackSource {
    id: Option<String>,
    pointer: String,
}

fn diagnostic_replaces_element(diagnostic: &CompatibilityDiagnostic) -> bool {
    if !diagnostic.effects().contains(&DiagnosticEffect::Visual) {
        return false;
    }
    let recovery = match diagnostic.details() {
        Some(ErrorDetails::Compatibility { recovery, .. })
        | Some(ErrorDetails::Asset { recovery, .. })
        | Some(ErrorDetails::MissingGlyph { recovery, .. })
        | Some(ErrorDetails::Metadata { recovery, .. })
        | Some(ErrorDetails::NestedSource { recovery, .. }) => Some(*recovery),
        Some(ErrorDetails::ResourceLimit { recovery, .. }) => *recovery,
        Some(ErrorDetails::PdfRasterFallback { recovery, .. }) => *recovery,
        _ => None,
    };
    matches!(recovery, Some(Recovery::Placeholder | Recovery::Omitted))
}

fn diagnostic_uses_non_generic_recovery(
    source: &NormalizedElement,
    diagnostic: &CompatibilityDiagnostic,
) -> bool {
    if !diagnostic_replaces_element(diagnostic) {
        return false;
    }
    let applies = diagnostic.element_id() == Some(source.source.id())
        || matches!(
            (&source.kind, diagnostic.details()),
            (
                NormalizedElementKind::Image(image),
                Some(ErrorDetails::Asset { asset_id, .. })
            ) if image.file_id.as_deref() == Some(asset_id.as_str())
        );
    if !applies {
        return false;
    }
    let recipe = match diagnostic.details() {
        Some(ErrorDetails::Compatibility { recipe_id, .. })
        | Some(ErrorDetails::Asset { recipe_id, .. }) => recipe_id.as_deref(),
        _ => None,
    };
    recipe != Some("renderer.known-element-unavailable.v1")
}

fn known_element_replaced_diagnostic(
    format: OutputFormat,
    element_id: &str,
    json_pointer: &str,
    has_bounds: bool,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(2)?;
    effects.extend_from_slice(&[DiagnosticEffect::Visual, DiagnosticEffect::Semantic]);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(
            "known element unavailable to this renderer used the revision-1 fallback",
        )?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery: if has_bounds {
                Recovery::Placeholder
            } else {
                Recovery::Omitted
            },
            recipe_id: has_bounds
                .then(|| try_owned("renderer.known-element-unavailable.v1"))
                .transpose()?,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn relationship_cycle_diagnostic(
    format: OutputFormat,
    element_id: &str,
    json_pointer: &str,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(2)?;
    effects.extend_from_slice(&[DiagnosticEffect::InputRepair, DiagnosticEffect::Semantic]);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::NORMALIZATION_RELATIONSHIP_CYCLE,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("relationship cycle-closing edge was removed deterministically")?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery: Recovery::Removed,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn fallback_font_diagnostic(
    code: DiagnosticCode,
    severity: DiagnosticSeverity,
    format: OutputFormat,
    element_id: Option<&str>,
    json_pointer: &str,
    missing_codepoint: Option<u32>,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(1)?;
    effects.push(DiagnosticEffect::Visual);
    Ok(CompatibilityDiagnostic {
        code,
        severity,
        format,
        effects,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(if missing_codepoint.is_some() {
            "generated fallback label used the deterministic missing-glyph replacement"
        } else {
            "generated fallback label used bundled Liberation Sans"
        })?,
        details: Some(Box::new(match missing_codepoint {
            Some(codepoint) => ErrorDetails::MissingGlyph {
                codepoint,
                recovery: Recovery::Substituted,
            },
            None => ErrorDetails::Compatibility {
                recovery: Recovery::Substituted,
                recipe_id: None,
            },
        })),
        order: DiagnosticOrder::default(),
    })
}

fn official_missing_glyph_diagnostic(
    format: OutputFormat,
    element_id: &str,
    json_pointer: &str,
    codepoint: u32,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut diagnostic = fallback_font_diagnostic(
        DiagnosticCode::FONT_MISSING_GLYPH,
        DiagnosticSeverity::Warning,
        format,
        Some(element_id),
        json_pointer,
        Some(codepoint),
    )?;
    diagnostic.message =
        try_owned("official text used the deterministic basic-Latin replacement glyph")?;
    Ok(diagnostic)
}

fn known_element_fallback(
    source: &NormalizedElement,
) -> Result<crate::scene_surface::SurfaceFallback, std::collections::TryReserveError> {
    let prefix = "Unavailable: ";
    let mut label = String::new();
    label.try_reserve_exact(prefix.len().saturating_add(source.kind_name.len()))?;
    label.push_str(prefix);
    label.push_str(source.kind_name);
    Ok(crate::scene_surface::SurfaceFallback {
        source_order: source.source_order,
        element_id: Some(try_owned(source.source.id())?),
        json_pointer: try_owned(source.source.pointer())?,
        label,
        link: source.link.as_deref().map(try_owned).transpose()?,
        frame_id: source.frame_id.as_deref().map(try_owned).transpose()?,
        is_frame: matches!(source.kind_name, "frame" | "magicframe"),
        x: source.x,
        y: source.y,
        width: source.width,
        height: source.height,
        angle: source.angle,
        opacity: source.opacity,
        paint: true,
    })
}

#[allow(clippy::too_many_arguments)]
fn include_transformed_bounds(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
    min_x: &mut f64,
    min_y: &mut f64,
    max_x: &mut f64,
    max_y: &mut f64,
) {
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let sin = angle.sin();
    let cos = angle.cos();
    for (corner_x, corner_y) in [
        (x, y),
        (x + width, y),
        (x + width, y + height),
        (x, y + height),
    ] {
        let delta_x = corner_x - center_x;
        let delta_y = corner_y - center_y;
        let rotated_x = center_x + delta_x * cos - delta_y * sin;
        let rotated_y = center_y + delta_x * sin + delta_y * cos;
        *min_x = min_x.min(rotated_x);
        *min_y = min_y.min(rotated_y);
        *max_x = max_x.max(rotated_x);
        *max_y = max_y.max(rotated_y);
    }
}

fn rough_drawing(
    source: &NormalizedElement,
    roughness: f64,
    budget: &mut rough::GenerationBudget,
) -> Result<rough::Drawing, rough::GenerationError> {
    use rough::{Point, RoundedSegment};

    let width = source.width;
    let height = source.height;
    let preserve_vertices = source.roundness.is_some() || source.roughness < 2.0;
    let fill_style =
        (source.background_color != "transparent").then_some(match source.fill_style.as_str() {
            "hachure" => rough::FillStyle::Hachure,
            "cross-hatch" => rough::FillStyle::CrossHatch,
            "zigzag" => rough::FillStyle::Zigzag,
            _ => rough::FillStyle::Solid,
        });
    let disable_multi_stroke = source.stroke_style != "solid";
    match &source.kind {
        NormalizedElementKind::Rectangle => {
            let Some(roundness) = source.roundness else {
                return rough::rectangle_drawing(
                    width,
                    height,
                    source.seed,
                    roughness,
                    preserve_vertices,
                    disable_multi_stroke,
                    fill_style,
                    source.stroke_width,
                    budget,
                );
            };
            let radius = corner_radius(width.min(height), roundness);
            let start = Point { x: radius, y: 0.0 };
            let segments = [
                RoundedSegment::Line(Point {
                    x: width - radius,
                    y: 0.0,
                }),
                RoundedSegment::Quadratic {
                    control: Point { x: width, y: 0.0 },
                    end: Point {
                        x: width,
                        y: radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: width,
                    y: height - radius,
                }),
                RoundedSegment::Quadratic {
                    control: Point {
                        x: width,
                        y: height,
                    },
                    end: Point {
                        x: width - radius,
                        y: height,
                    },
                },
                RoundedSegment::Line(Point {
                    x: radius,
                    y: height,
                }),
                RoundedSegment::Quadratic {
                    control: Point { x: 0.0, y: height },
                    end: Point {
                        x: 0.0,
                        y: height - radius,
                    },
                },
                RoundedSegment::Line(Point { x: 0.0, y: radius }),
                RoundedSegment::Quadratic {
                    control: Point { x: 0.0, y: 0.0 },
                    end: Point { x: radius, y: 0.0 },
                },
            ];
            let fill_points = rough::rounded_fill_points(start, &segments, roughness, budget)?;
            rough::rounded_drawing(
                start,
                &segments,
                &fill_points,
                source.seed,
                roughness,
                preserve_vertices,
                disable_multi_stroke,
                fill_style,
                source.stroke_width,
                budget,
            )
        }
        NormalizedElementKind::Diamond => {
            let points = [
                Point {
                    x: width / 2.0,
                    y: 0.0,
                },
                Point {
                    x: width,
                    y: height / 2.0,
                },
                Point {
                    x: width / 2.0,
                    y: height,
                },
                Point {
                    x: 0.0,
                    y: height / 2.0,
                },
            ];
            let Some(roundness) = source.roundness else {
                return rough::polygon_drawing(
                    &points,
                    source.seed,
                    roughness,
                    preserve_vertices,
                    disable_multi_stroke,
                    fill_style,
                    source.stroke_width,
                    budget,
                );
            };
            let vertical_radius = corner_radius(width / 2.0, roundness);
            let horizontal_radius = corner_radius(height / 2.0, roundness);
            let [top, right, bottom, left] = points;
            let start = Point {
                x: top.x + vertical_radius,
                y: top.y + horizontal_radius,
            };
            let segments = [
                RoundedSegment::Line(Point {
                    x: right.x - vertical_radius,
                    y: right.y - horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: right,
                    control_2: right,
                    end: Point {
                        x: right.x - vertical_radius,
                        y: right.y + horizontal_radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: bottom.x + vertical_radius,
                    y: bottom.y - horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: bottom,
                    control_2: bottom,
                    end: Point {
                        x: bottom.x - vertical_radius,
                        y: bottom.y - horizontal_radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: left.x + vertical_radius,
                    y: left.y + horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: left,
                    control_2: left,
                    end: Point {
                        x: left.x + vertical_radius,
                        y: left.y - horizontal_radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: top.x - vertical_radius,
                    y: top.y + horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: top,
                    control_2: top,
                    end: Point {
                        x: top.x + vertical_radius,
                        y: top.y + horizontal_radius,
                    },
                },
            ];
            let fill_points = rough::rounded_fill_points(start, &segments, roughness, budget)?;
            rough::rounded_drawing(
                start,
                &segments,
                &fill_points,
                source.seed,
                roughness,
                preserve_vertices,
                disable_multi_stroke,
                fill_style,
                source.stroke_width,
                budget,
            )
        }
        NormalizedElementKind::Ellipse => rough::ellipse_drawing(
            width,
            height,
            source.seed,
            roughness,
            preserve_vertices,
            disable_multi_stroke,
            fill_style,
            source.stroke_width,
            budget,
        ),
        NormalizedElementKind::Linear(linear) => {
            let mut points = Vec::new();
            budget.work(u64::try_from(linear.points.len()).unwrap_or(u64::MAX))?;
            points.try_reserve_exact(linear.points.len())?;
            points.extend(linear.points.iter().map(|point| Point {
                x: point[0],
                y: point[1],
            }));
            const LINE_CONFIRM_THRESHOLD: f64 = 8.0;
            let closed = points.len() >= 3
                && points
                    .first()
                    .zip(points.last())
                    .is_some_and(|(first, last)| {
                        (first.x - last.x).hypot(first.y - last.y) <= LINE_CONFIRM_THRESHOLD
                    });
            let mut drawing = if linear.elbowed {
                elbow_drawing(
                    &points,
                    source.seed,
                    roughness,
                    disable_multi_stroke,
                    budget,
                )?
            } else if source.roundness.is_some() {
                rough::curve_drawing(
                    &points,
                    closed,
                    source.seed,
                    roughness,
                    disable_multi_stroke,
                    closed.then_some(fill_style).flatten(),
                    source.stroke_width,
                    budget,
                )?
            } else {
                if closed && fill_style.is_some() {
                    rough::polygon_drawing(
                        &points,
                        source.seed,
                        roughness,
                        preserve_vertices,
                        disable_multi_stroke,
                        fill_style,
                        source.stroke_width,
                        budget,
                    )?
                } else {
                    rough::linear_drawing(
                        &points,
                        closed,
                        source.seed,
                        roughness,
                        preserve_vertices,
                        disable_multi_stroke,
                        None,
                        source.stroke_width,
                        budget,
                    )?
                }
            };
            if matches!(
                linear.kind,
                crate::normalization::NormalizedLinearKind::Arrow
            ) {
                add_arrowheads(&mut drawing, linear, source, roughness, budget)?;
            }
            Ok(drawing)
        }
        NormalizedElementKind::Freedraw(_)
        | NormalizedElementKind::Text(_)
        | NormalizedElementKind::Image(_)
        | NormalizedElementKind::Frame(_)
        | NormalizedElementKind::MagicFrame(_) => {
            unreachable!("only outlines reach Rough geometry")
        }
    }
}

fn elbow_drawing(
    points: &[rough::Point],
    seed: i32,
    roughness: f64,
    disable_multi_stroke: bool,
    budget: &mut rough::GenerationBudget,
) -> Result<rough::Drawing, rough::GenerationError> {
    use rough::{Point, RoundedSegment};

    let start = points[0];
    let mut segments = Vec::new();
    segments.try_reserve_exact(points.len().saturating_mul(2))?;
    for index in 1..points.len().saturating_sub(1) {
        let previous = points[index - 1];
        let corner = points[index];
        let next = points[index + 1];
        let radius = 16.0_f64
            .min((corner.x - previous.x).hypot(corner.y - previous.y) / 2.0)
            .min((next.x - corner.x).hypot(next.y - corner.y) / 2.0);
        let previous_is_horizontal = (previous.x - corner.x).abs() >= (previous.y - corner.y).abs();
        let next_is_horizontal = (next.x - corner.x).abs() >= (next.y - corner.y).abs();
        let approach = if previous_is_horizontal {
            rough::Point {
                x: corner.x
                    + if previous.x < corner.x {
                        -radius
                    } else {
                        radius
                    },
                y: corner.y,
            }
        } else {
            rough::Point {
                x: corner.x,
                y: corner.y
                    + if previous.y < corner.y {
                        -radius
                    } else {
                        radius
                    },
            }
        };
        let departure = if next_is_horizontal {
            rough::Point {
                x: corner.x + if next.x < corner.x { -radius } else { radius },
                y: corner.y,
            }
        } else {
            rough::Point {
                x: corner.x,
                y: corner.y + if next.y < corner.y { -radius } else { radius },
            }
        };
        segments.push(RoundedSegment::Line(approach));
        segments.push(RoundedSegment::Quadratic {
            control: corner,
            end: departure,
        });
    }
    segments.push(RoundedSegment::Line(
        *points.last().unwrap_or(&Point { x: 0.0, y: 0.0 }),
    ));
    rough::rounded_drawing(
        start,
        &segments,
        points,
        seed,
        roughness,
        true,
        disable_multi_stroke,
        None,
        1.0,
        budget,
    )
}

fn add_arrowheads(
    drawing: &mut rough::Drawing,
    linear: &crate::normalization::NormalizedLinear,
    source: &NormalizedElement,
    roughness: f64,
    budget: &mut rough::GenerationBudget,
) -> Result<(), rough::GenerationError> {
    let mut arrowhead_roughness = roughness;
    for (start, arrowhead) in [
        (true, linear.start_arrowhead.as_deref()),
        (false, linear.end_arrowhead.as_deref()),
    ] {
        let Some(arrowhead) = arrowhead else { continue };
        let Some(points) = arrowhead_points(drawing, linear, start, arrowhead, source.stroke_width)
        else {
            continue;
        };
        arrowhead_roughness = arrowhead_roughness.min(
            if matches!(arrowhead, "dot" | "circle" | "circle_outline") {
                0.5
            } else {
                1.0
            },
        );
        let head_roughness = arrowhead_roughness;
        let preserve_vertices = source.roughness < 2.0;
        match arrowhead {
            "dot" | "circle" | "circle_outline" => {
                let [center_x, center_y, diameter, ..] = points;
                let mut head = rough::circle_drawing(
                    diameter,
                    source.seed,
                    head_roughness,
                    preserve_vertices,
                    source.stroke_style != "solid",
                    Some(rough::FillStyle::Solid),
                    source.stroke_width,
                    budget,
                )?;
                translate_drawing(
                    &mut head,
                    center_x - diameter / 2.0,
                    center_y - diameter / 2.0,
                );
                drawing.decorations.push(rough::Decoration {
                    drawing: head,
                    fill_paint: if arrowhead == "circle_outline" {
                        rough::DecorationFillPaint::Background
                    } else {
                        rough::DecorationFillPaint::Stroke
                    },
                    dotted: false,
                });
            }
            "triangle" | "triangle_outline" => {
                let [tip_x, tip_y, left_x, left_y, right_x, right_y, ..] = points;
                let head_points = [
                    rough::Point { x: tip_x, y: tip_y },
                    rough::Point {
                        x: left_x,
                        y: left_y,
                    },
                    rough::Point {
                        x: right_x,
                        y: right_y,
                    },
                    rough::Point { x: tip_x, y: tip_y },
                ];
                let head = rough::polygon_drawing(
                    &head_points,
                    source.seed,
                    head_roughness,
                    preserve_vertices,
                    source.stroke_style != "solid",
                    Some(rough::FillStyle::Solid),
                    source.stroke_width,
                    budget,
                )?;
                drawing.decorations.push(rough::Decoration {
                    drawing: head,
                    fill_paint: if arrowhead == "triangle_outline" {
                        rough::DecorationFillPaint::Background
                    } else {
                        rough::DecorationFillPaint::Stroke
                    },
                    dotted: false,
                });
            }
            "diamond" | "diamond_outline" => {
                let head_points = [
                    rough::Point {
                        x: points[0],
                        y: points[1],
                    },
                    rough::Point {
                        x: points[2],
                        y: points[3],
                    },
                    rough::Point {
                        x: points[4],
                        y: points[5],
                    },
                    rough::Point {
                        x: points[6],
                        y: points[7],
                    },
                    rough::Point {
                        x: points[0],
                        y: points[1],
                    },
                ];
                let head = rough::polygon_drawing(
                    &head_points,
                    source.seed,
                    head_roughness,
                    preserve_vertices,
                    source.stroke_style != "solid",
                    Some(rough::FillStyle::Solid),
                    source.stroke_width,
                    budget,
                )?;
                drawing.decorations.push(rough::Decoration {
                    drawing: head,
                    fill_paint: if arrowhead == "diamond_outline" {
                        rough::DecorationFillPaint::Background
                    } else {
                        rough::DecorationFillPaint::Stroke
                    },
                    dotted: false,
                });
            }
            _ => {
                let crossbar = (arrowhead == "crowfoot_one_or_many")
                    .then(|| {
                        arrowhead_points(
                            drawing,
                            linear,
                            start,
                            "crowfoot_one",
                            source.stroke_width,
                        )
                    })
                    .flatten();
                add_open_arrowhead(drawing, source, arrowhead, points, head_roughness, budget)?;
                if let Some(crossbar) = crossbar {
                    add_open_arrowhead(
                        drawing,
                        source,
                        "crowfoot_one",
                        crossbar,
                        head_roughness,
                        budget,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn add_open_arrowhead(
    drawing: &mut rough::Drawing,
    source: &NormalizedElement,
    arrowhead: &str,
    points: [f64; 8],
    roughness: f64,
    budget: &mut rough::GenerationBudget,
) -> Result<(), rough::GenerationError> {
    let tip = rough::Point {
        x: points[0],
        y: points[1],
    };
    let first = rough::Point {
        x: points[2],
        y: points[3],
    };
    let second = rough::Point {
        x: points[4],
        y: points[5],
    };
    let mut push_line = |start, end| -> Result<(), rough::GenerationError> {
        let head = rough::linear_drawing(
            &[start, end],
            false,
            source.seed,
            roughness,
            source.roughness < 2.0,
            source.stroke_style != "solid",
            None,
            source.stroke_width,
            budget,
        )?;
        drawing.decorations.push(rough::Decoration {
            drawing: head,
            fill_paint: rough::DecorationFillPaint::Stroke,
            dotted: source.stroke_style == "dotted",
        });
        Ok(())
    };
    if arrowhead == "crowfoot_one" {
        push_line(first, second)?;
    } else {
        push_line(first, tip)?;
        push_line(second, tip)?;
    }
    Ok(())
}

fn arrowhead_points(
    drawing: &rough::Drawing,
    linear: &crate::normalization::NormalizedLinear,
    start: bool,
    arrowhead: &str,
    stroke_width: f64,
) -> Option<[f64; 8]> {
    use rough::PathOperation;
    let operations = &drawing.outline.operations;
    let index = if start {
        1
    } else {
        operations.len().checked_sub(1)?
    };
    let PathOperation::Cubic {
        control_1,
        control_2,
        end,
    } = operations.get(index)?
    else {
        return None;
    };
    let p0 = match operations.get(index.checked_sub(1)?)? {
        PathOperation::Move(point) => *point,
        PathOperation::Cubic { end, .. } => *end,
        PathOperation::Line(point) => *point,
    };
    let p3 = *end;
    let equation = |coordinate: fn(rough::Point) -> f64| {
        let t = 0.3_f64;
        (1.0 - t).powi(3) * coordinate(p3)
            + 3.0 * t * (1.0 - t).powi(2) * coordinate(*control_2)
            + 3.0 * t.powi(2) * (1.0 - t) * coordinate(*control_1)
            + t.powi(3) * coordinate(p0)
    };
    let near = rough::Point {
        x: equation(|p| p.x),
        y: equation(|p| p.y),
    };
    let tip = if start { p0 } else { p3 };
    let distance = (tip.x - near.x).hypot(tip.y - near.y);
    if distance == 0.0 {
        return None;
    }
    let normal = rough::Point {
        x: (tip.x - near.x) / distance,
        y: (tip.y - near.y) / distance,
    };
    let size: f64 = match arrowhead {
        "arrow" => 25.0,
        "diamond" | "diamond_outline" => 12.0,
        "crowfoot_many" | "crowfoot_one" | "crowfoot_one_or_many" => 20.0,
        _ => 15.0,
    };
    let endpoint_index = if start { 0 } else { linear.points.len() - 1 };
    let adjacent_index = if start { 1 } else { linear.points.len() - 2 };
    let segment_length = (linear.points[endpoint_index][0] - linear.points[adjacent_index][0])
        .hypot(linear.points[endpoint_index][1] - linear.points[adjacent_index][1]);
    let minimum = size.min(
        segment_length
            * if arrowhead.starts_with("diamond") {
                0.25
            } else {
                0.5
            },
    );
    let base = rough::Point {
        x: tip.x - normal.x * minimum,
        y: tip.y - normal.y * minimum,
    };
    if matches!(arrowhead, "dot" | "circle" | "circle_outline") {
        return Some([
            tip.x,
            tip.y,
            minimum + stroke_width - 2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ]);
    }
    let angle = if arrowhead == "bar" {
        90.0
    } else if arrowhead == "arrow" {
        20.0
    } else {
        25.0
    };
    let rotate = |point: rough::Point, center: rough::Point, degrees: f64| {
        let radians = degrees.to_radians();
        let dx = point.x - center.x;
        let dy = point.y - center.y;
        rough::Point {
            x: center.x + dx * radians.cos() - dy * radians.sin(),
            y: center.y + dx * radians.sin() + dy * radians.cos(),
        }
    };
    if matches!(arrowhead, "crowfoot_many" | "crowfoot_one_or_many") {
        let first = rotate(tip, base, -angle);
        let second = rotate(tip, base, angle);
        return Some([
            base.x, base.y, first.x, first.y, second.x, second.y, 0.0, 0.0,
        ]);
    }
    let first = rotate(base, tip, -angle);
    let second = rotate(base, tip, angle);
    if matches!(arrowhead, "diamond" | "diamond_outline") {
        let adjacent = linear.points[adjacent_index];
        let direction = if start {
            (adjacent[1] - tip.y).atan2(adjacent[0] - tip.x)
        } else {
            (tip.y - adjacent[1]).atan2(tip.x - adjacent[0])
        };
        let opposite = rough::Point {
            x: tip.x + if start { minimum * 2.0 } else { -minimum * 2.0 } * direction.cos(),
            y: tip.y + if start { minimum * 2.0 } else { -minimum * 2.0 } * direction.sin(),
        };
        return Some([
            tip.x, tip.y, first.x, first.y, opposite.x, opposite.y, second.x, second.y,
        ]);
    }
    Some([tip.x, tip.y, first.x, first.y, second.x, second.y, 0.0, 0.0])
}

fn translate_drawing(drawing: &mut rough::Drawing, x: f64, y: f64) {
    fn translate(path: &mut rough::Path, x: f64, y: f64) {
        for operation in &mut path.operations {
            match operation {
                rough::PathOperation::Move(point) | rough::PathOperation::Line(point) => {
                    point.x += x;
                    point.y += y;
                }
                rough::PathOperation::Cubic {
                    control_1,
                    control_2,
                    end,
                } => {
                    for point in [control_1, control_2, end] {
                        point.x += x;
                        point.y += y;
                    }
                }
            }
        }
    }
    translate(&mut drawing.outline, x, y);
    if let Some(fill) = &mut drawing.fill {
        translate(&mut fill.path, x, y);
    }
}

fn local_outline_bounds(
    source: &NormalizedElement,
    drawing: Option<&rough::Drawing>,
) -> (f64, f64, f64, f64) {
    let NormalizedElementKind::Linear(linear) = &source.kind else {
        return (0.0, 0.0, source.width, source.height);
    };
    if let Some(bounds) = drawing.and_then(|drawing| drawing.outline.cubic_bounds()) {
        return bounds;
    }
    linear.points.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), point| {
            (
                min_x.min(point[0]),
                min_y.min(point[1]),
                max_x.max(point[0]),
                max_y.max(point[1]),
            )
        },
    )
}

fn union_bounds(
    first: Option<(f64, f64, f64, f64)>,
    second: Option<(f64, f64, f64, f64)>,
) -> Option<(f64, f64, f64, f64)> {
    match (first, second) {
        (Some(first), Some(second)) => Some((
            first.0.min(second.0),
            first.1.min(second.1),
            first.2.max(second.2),
            first.3.max(second.3),
        )),
        (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
        (None, None) => None,
    }
}

fn corner_radius(size: f64, roundness: crate::normalization::NormalizedRoundness) -> f64 {
    const PROPORTIONAL_RADIUS: f64 = 0.25;
    match roundness {
        crate::normalization::NormalizedRoundness::Proportional => size * PROPORTIONAL_RADIUS,
        crate::normalization::NormalizedRoundness::Adaptive { radius } => {
            radius.min(size * PROPORTIONAL_RADIUS)
        }
    }
}

struct SemanticWorkingRecord {
    source_order: u64,
    event: &'static str,
    element_id: Option<String>,
    local_owner: Option<String>,
    json_pointer: String,
    bytes: u64,
}

#[derive(Clone, Copy)]
struct ResolvedTransform {
    translation_x: f64,
    translation_y: f64,
    angle_radians: f64,
    center_x: f64,
    center_y: f64,
}

struct ElementLocalCrossing {
    code: DiagnosticCode,
    field: &'static str,
    configured: ResourceValue,
    value_kind: ValueKind,
    attempted: ResourceValue,
    pointer_field: Option<&'static str>,
    message: &'static str,
}

impl ElementLocalCrossing {
    fn diagnostic(
        self,
        format: OutputFormat,
        element_id: Option<&str>,
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
            element_id: element_id.map(try_owned).transpose()?,
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

fn outline_local_crossing(
    source: &NormalizedElement,
    request: &RenderRequest,
    geometry_segments: u64,
    element_work_units: u64,
) -> Option<ElementLocalCrossing> {
    let coordinate_limit = request.resource_limits().max_coordinate_magnitude();
    for (value, pointer_field) in [
        (source.x, "x"),
        (source.y, "y"),
        (source.width, "width"),
        (source.height, "height"),
    ] {
        let magnitude = value.abs();
        if magnitude > coordinate_limit {
            return Some(ElementLocalCrossing {
                code: DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED,
                field: "max_coordinate_magnitude",
                configured: ResourceValue::Finite(FiniteValue::new(coordinate_limit)?),
                value_kind: ValueKind::Coordinate,
                attempted: ResourceValue::Finite(FiniteValue::new(magnitude)?),
                pointer_field: Some(pointer_field),
                message: "outline omitted because a coordinate exceeds its limit",
            });
        }
    }
    let points = match &source.kind {
        NormalizedElementKind::Linear(linear) => Some(linear.points.as_slice()),
        NormalizedElementKind::Freedraw(freedraw) => Some(freedraw.points.as_slice()),
        _ => None,
    };
    if let Some(points) = points {
        for point in points {
            for value in point {
                let magnitude = value.abs();
                if magnitude > coordinate_limit {
                    return Some(ElementLocalCrossing {
                        code: DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED,
                        field: "max_coordinate_magnitude",
                        configured: ResourceValue::Finite(FiniteValue::new(coordinate_limit)?),
                        value_kind: ValueKind::Coordinate,
                        attempted: ResourceValue::Finite(FiniteValue::new(magnitude)?),
                        pointer_field: Some("points"),
                        message: "outline omitted because a coordinate exceeds its limit",
                    });
                }
            }
        }
    }

    let geometry_limit = u64::from(
        request
            .resource_limits()
            .max_geometry_segments_per_element(),
    );
    if geometry_segments > geometry_limit {
        return Some(ElementLocalCrossing {
            code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
            field: "max_geometry_segments_per_element",
            configured: ResourceValue::Integer(geometry_limit),
            value_kind: ValueKind::Count,
            attempted: ResourceValue::Integer(geometry_segments),
            pointer_field: None,
            message: "outline omitted because its geometry exceeds its limit",
        });
    }

    let work_limit = request.resource_limits().max_element_work_units();
    (element_work_units > work_limit).then_some(ElementLocalCrossing {
        code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
        field: "max_element_work_units",
        configured: ResourceValue::Integer(work_limit),
        value_kind: ValueKind::WorkUnits,
        attempted: ResourceValue::Integer(element_work_units),
        pointer_field: None,
        message: "outline omitted because its semantic work exceeds its limit",
    })
}

fn generation_local_crossing(
    kind: rough::GenerationLimit,
    attempted: u64,
    request: &RenderRequest,
) -> ElementLocalCrossing {
    match kind {
        rough::GenerationLimit::GeometrySegments => ElementLocalCrossing {
            code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
            field: "max_geometry_segments_per_element",
            configured: ResourceValue::Integer(u64::from(
                request
                    .resource_limits()
                    .max_geometry_segments_per_element(),
            )),
            value_kind: ValueKind::Count,
            attempted: ResourceValue::Integer(attempted),
            pointer_field: None,
            message: "outline omitted because its geometry exceeds its limit",
        },
        rough::GenerationLimit::WorkUnits => ElementLocalCrossing {
            code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
            field: "max_element_work_units",
            configured: ResourceValue::Integer(request.resource_limits().max_element_work_units()),
            value_kind: ValueKind::WorkUnits,
            attempted: ResourceValue::Integer(attempted),
            pointer_field: None,
            message: "outline omitted because its semantic work exceeds its limit",
        },
    }
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
        assert_eq!(approval["approved_at"], "2026-07-22");
        assert_eq!(
            approval["approval_url"],
            "https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5051200664"
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
            let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
            let mut session = RenderSession::new(
                profile,
                crate::catalog::REVISION_1.scene_surface(),
                &scene,
                &request,
            );
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
            let mut retained = diagnostic;
            retained.order.owner_index = scene.source_owner_order("elements", 0);
            retained.order.retention_ordinal = 0;
            assert_eq!(error.report().diagnostics(), &[retained]);
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
        let mut session = RenderSession::new(
            profile,
            crate::catalog::REVISION_1.scene_surface(),
            &scene,
            &request,
        );
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
        let mut retained = diagnostic;
        retained.order.owner_index = scene.source_owner_order("elements", 0);
        retained.order.retention_ordinal = 0;
        assert_eq!(error.report().diagnostics(), &[retained]);
    }

    #[test]
    fn diagnostic_trace_uses_the_callers_canonical_phase() {
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("request is valid");
        let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");
        let mut session = RenderSession::new_with_trace(
            profile,
            crate::catalog::REVISION_1.scene_surface(),
            &scene,
            &request,
        );

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
                "6c53083b5df3852ac5f6d566047d76e36cc53da008efe0eaed427915b461b5e5",
            ),
            (
                OutputFormat::Png,
                "3f96fd0444186a0419c2a785f4ec851b969c0fa029a7d918997ccf58394a3190",
            ),
            (
                OutputFormat::Pdf,
                "9f2c32bbb479856363721973d7e60b0cfac1688952cdb2eebdf46f245ee31089",
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
            crate::normalization::NORMALIZATION_WORK_UNITS + 8
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
        let scene = SceneDocument::parse(EMPTY_SCENE).expect("scene is valid");
        let mut session = RenderSession::new(
            profile,
            crate::catalog::REVISION_1.scene_surface(),
            &scene,
            &request,
        );
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

    #[test]
    fn diagnosed_unbounded_omission_has_its_own_semantic_transaction() {
        let scene = SceneDocument::parse(
            br#"{"type":"excalidraw","version":2,"elements":[null],"appState":{},"files":{}}"#,
        )
        .expect("omission vector is valid");
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("request is valid");
        let profile = Renderer::profile_identity(CompatibilityRevision::REVISION_1)
            .expect("revision 1 exists");

        let (result, trace) = render_with_trace(profile, &scene, &request);
        result.expect("diagnosed omission retains a meaningful artifact");
        let trace: serde_json::Value = serde_json::from_slice(&trace).expect("trace is valid JSON");
        let omissions = trace["entries"]
            .as_array()
            .expect("trace entries are an array")
            .iter()
            .filter(|entry| entry["event"] == "compatibility_omission")
            .collect::<Vec<_>>();
        let [omission] = omissions.as_slice() else {
            panic!("one omission transaction is retained");
        };
        assert_eq!(omission["json_pointer"], "/elements/0");
        assert_eq!(omission["deltas"]["total_work_units"], 8);
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
