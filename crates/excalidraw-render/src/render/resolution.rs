//! Scene resolution: the phases of [`RenderSession::resolve_scene`] that turn
//! the admitted scene into the format-neutral [`ResolvedScene`] the encoder
//! consumes. Phase order — inputs, census, element resolution, assembly — is the
//! canonical charge order (ADR-0002); the journal-replay-vs-live and
//! scene-vs-frame variants end inside [`RenderSession::acquire_resolution_inputs`].

use crate::diagnostic_registry::LocalRecovery;
use crate::normalization::{
    normalize_scene_with_assets, NormalizationError, NormalizationOutcome, NormalizedElement,
    NormalizedElementKind, NormalizedFileRecord, NormalizedFreedraw, NormalizedLinear,
};
use crate::outcome::DiagnosticAttribution;
use crate::relationships::{self, RelationshipError, RelationshipOutcome};
use crate::resource_accounting::{ResourceContext, ResourceDelta, ResourcePhase};
use crate::scene::{self, SourceField};
use crate::scene_surface::SurfaceFallback;
use crate::{OutputFormat, RenderError, RenderRequest, SceneDocument};

use super::diagnostics::{
    diagnostic_uses_non_generic_recovery, known_element_fallback,
    known_element_replaced_diagnostic, relationship_cycle_diagnostic,
};
use super::frame_target::{
    selected_file_ids, EffectiveFrameRendering, FrameSelection, FrameViewport,
};
use super::geometry::{adjusted_roughness, rough_drawing, GeneratedFreedraw, GeneratedGeometry};
use super::prepared::PreparedFrameRender;
use super::resolve::{
    fallback_fits_local_limits, format_error_code, generation_local_crossing,
    logical_aligned_bytes, outline_local_crossing, relationship_asset_authorities,
    ResolveSceneError, ResolvedScene, SemanticWorkingRecord,
};
use super::{
    asset, element_owner_index, freedraw, image, rough, try_owned, DiagnosticOwnership,
    FormatEncodeError, RenderSession, RenderTarget,
};

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

/// Relationship resolution for one render: replayed from a prepared frame
/// journal, or computed live and owned here. Dereferences to the outcome so
/// later phases are indifferent to which variant produced it.
enum ResolvedRelationships<'p> {
    Replayed(&'p RelationshipOutcome),
    Live(RelationshipOutcome),
}

impl std::ops::Deref for ResolvedRelationships<'_> {
    type Target = RelationshipOutcome;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Replayed(relationships) => relationships,
            Self::Live(relationships) => relationships,
        }
    }
}

/// Everything phase 1 settles: the frame-scoped normalization, the resolved
/// relationships, and the target scope. The journal-replay-vs-live and
/// scene-vs-frame variants never escape `acquire_resolution_inputs`.
struct ResolutionInputs<'p> {
    normalization: NormalizationOutcome,
    relationships: ResolvedRelationships<'p>,
    frame_viewport: Option<FrameViewport>,
    frame_participating_sources: Option<Vec<usize>>,
    selected_file_ids: Option<Vec<String>>,
}

/// The non-element scene state the assembly phase needs: file records, view
/// background, effective frame rendering, and the target viewport.
struct SceneChrome {
    files: Vec<NormalizedFileRecord>,
    view_background_color: String,
    view_background_working_bytes: u64,
    frame_rendering: EffectiveFrameRendering,
    viewport: Option<FrameViewport>,
}

impl RenderSession {
    /// Normalizes, relates, frame-scopes, and resolves the scene into the
    /// format-independent plan the encoder consumes. Phase order — inputs,
    /// census, element resolution, assembly — is the canonical charge order.
    pub(super) fn resolve_scene(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        prepared: Option<&PreparedFrameRender>,
        semantic_working: &mut Vec<SemanticWorkingRecord>,
    ) -> Result<ResolvedScene, RenderError> {
        let inputs = self.acquire_resolution_inputs(scene, request, prepared)?;
        let mut images = image::ImageResolver::new(
            relationship_asset_authorities(&inputs.relationships.graph).map_err(|_| {
                self.memory_failure(request, "relationship_resolution", "asset_authorities")
            })?,
        );
        self.account_semantic_census(scene, request, &inputs, semantic_working)?;
        let frame_rendering = inputs.normalization.frame_rendering;
        let view_background_color = inputs.normalization.view_background_color;
        let view_background_working_bytes = inputs.normalization.view_background_working_bytes;
        let effective_frame_rendering = match self.target {
            RenderTarget::Scene => EffectiveFrameRendering {
                enabled: frame_rendering.enabled && request.frame_options().enabled(),
                clip: frame_rendering.clip && request.frame_options().clip(),
                name: frame_rendering.name && request.frame_options().name(),
                outline: frame_rendering.outline && request.frame_options().outline(),
                rectangular_clip: false,
                emit_frame_elements: true,
            },
            RenderTarget::Frame(_) => EffectiveFrameRendering {
                enabled: true,
                clip: request.format() != OutputFormat::Png,
                name: false,
                outline: false,
                rectangular_clip: true,
                emit_frame_elements: false,
            },
        };
        let chrome = SceneChrome {
            files: inputs.normalization.files,
            view_background_color,
            view_background_working_bytes,
            frame_rendering: effective_frame_rendering,
            viewport: inputs.frame_viewport,
        };
        let mut fallbacks = inputs.normalization.fallbacks;
        let mut outlines = Vec::new();
        outlines
            .try_reserve_exact(inputs.normalization.elements.len())
            .map_err(|_| self.memory_failure(request, "normalization", "outline_list"))?;
        for mut source in inputs.normalization.elements {
            debug_assert!(!source.index.is_empty());
            if matches!(source.kind, NormalizedElementKind::Image(_)) {
                match images.resolve_element(self, scene, request, source, semantic_working)? {
                    image::ImageDisposition::Outline(element) => {
                        outlines.push((element, None));
                        continue;
                    }
                    image::ImageDisposition::Generic(element) => source = element,
                }
            }
            if let NormalizedElementKind::Embeddable { iframe, .. } = &source.kind {
                let both_transparent = source.stroke_color == "transparent"
                    && source.background_color == "transparent";
                if both_transparent {
                    source.background_color = try_owned("#d3d3d3").map_err(|_| {
                        self.memory_failure(request, "normalization", "embeddable_color")
                    })?;
                    source.fill_style = try_owned("solid").map_err(|_| {
                        self.memory_failure(request, "normalization", "embeddable_fill")
                    })?;
                    source.roughness = 0.0;
                } else if *iframe {
                    if source.background_color == "transparent" {
                        source.background_color = try_owned("#f4f4f6").map_err(|_| {
                            self.memory_failure(request, "normalization", "iframe_background")
                        })?;
                    }
                    if source.stroke_color == "transparent" {
                        source.stroke_color = try_owned("#000000").map_err(|_| {
                            self.memory_failure(request, "normalization", "iframe_stroke")
                        })?;
                    }
                }
            }
            let is_outline = matches!(
                &source.kind,
                NormalizedElementKind::Rectangle
                    | NormalizedElementKind::Diamond
                    | NormalizedElementKind::Ellipse
                    | NormalizedElementKind::Embeddable { .. }
                    | NormalizedElementKind::Linear(NormalizedLinear {
                        geometry_available: true,
                        ..
                    })
                    | NormalizedElementKind::Freedraw(NormalizedFreedraw {
                        geometry_available: true,
                        ..
                    })
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
                if self.preflight_official_text(
                    &source,
                    request,
                    semantic_working,
                    &mut fallbacks,
                )? {
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
                    diagnostic_uses_non_generic_recovery(&source, diagnostic)
                        || diagnostic.element_id() == Some(source.source.id())
                            && diagnostic.replaces_element()
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
                        self.memory_failure(request, "semantic_geometry", "known_element_fallback")
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
                NormalizedElementKind::Embeddable { .. } => true,
                _ => source.width != 0.0 && source.height != 0.0,
            };
            let base_element_work_units = source
                .normalization_work_units
                .saturating_add(8)
                .saturating_add(source.repair_work_units);
            // Coordinate and already-spent work admission must precede any Rough allocation.
            let mut local_crossing =
                outline_local_crossing(&source, request, 0, base_element_work_units);
            let mut rejected_geometry_segments = 0;
            let geometry = if has_outline && local_crossing.is_none() {
                let roughness = adjusted_roughness(&source);
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
                                let background_fill = if source.background_color == "transparent" {
                                    None
                                } else {
                                    freedraw::background_fill(
                                        &freedraw.points,
                                        source.seed,
                                        roughness,
                                        source.stroke_style != "solid",
                                        rough::FillStyle::from_normalized(&source.fill_style),
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
                    _ => {
                        rough_drawing(&source, roughness, &mut budget).map(GeneratedGeometry::Rough)
                    }
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
                    Err(rough::GenerationError::Limit {
                        kind,
                        attempted,
                        geometry_segments,
                    }) => {
                        rejected_geometry_segments = geometry_segments;
                        local_crossing = Some(generation_local_crossing(kind, attempted, request));
                        None
                    }
                }
            } else {
                None
            };
            let geometry_segments = geometry
                .as_ref()
                .map_or(rejected_geometry_segments, |geometry| {
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
                NormalizedElementKind::Embeddable { .. } => "rough_embeddable_geometry",
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
            let element_work_units = base_element_work_units.saturating_add(geometry_work_units);
            if local_crossing.is_none() {
                local_crossing =
                    outline_local_crossing(&source, request, geometry_segments, element_work_units);
            }
            let include_outline = has_outline && local_crossing.is_none();
            let semantic_working_bytes = if let Some(crossing) = local_crossing {
                self.commit_local_rejection_observation(
                    request,
                    geometry_context,
                    ResourceDelta {
                        geometry_segments,
                        ..ResourceDelta::default()
                    },
                )?;
                let source_position = crossing
                    .pointer_field
                    .map_or(u64::MAX, |field| source.source.coordinate_position(field));
                let replacement = if source.width > 0.0 && source.height > 0.0 {
                    let fallback = known_element_fallback(&source).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "known_element_fallback")
                    })?;
                    fallback_fits_local_limits(&fallback, request.resource_limits())
                        .then_some(fallback)
                } else {
                    None
                };
                let can_replace = replacement.is_some();
                let diagnostic = crossing
                    .diagnostic_with_recovery(
                        request.format(),
                        Some(source.source.id()),
                        source.source.pointer(),
                        LocalRecovery::placeholder_if(can_replace),
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
                if let Some(fallback) = replacement {
                    fallbacks.try_reserve(1).map_err(|_| {
                        self.memory_failure(request, "semantic_geometry", "fallback_list")
                    })?;
                    fallbacks.push(fallback);
                } else {
                    self.charge_local_omission(
                        request,
                        Some(source.source.id()),
                        source.source.pointer(),
                        "outline_local_omission",
                    )?;
                }
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
        self.assemble_resolved_scene(
            scene,
            request,
            outlines,
            images,
            fallbacks,
            chrome,
            &inputs.relationships,
            semantic_working,
        )
    }

    /// Phase 1: obtains the normalized scene and its relationships — replayed
    /// from the prepared journal or computed live — and applies frame scoping.
    /// Both variation axes (journal-replay vs live, scene vs frame target)
    /// end here; later phases see only [`ResolutionInputs`].
    fn acquire_resolution_inputs<'p>(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        prepared: Option<&'p PreparedFrameRender>,
    ) -> Result<ResolutionInputs<'p>, RenderError> {
        let (mut normalization, relationships) = if let Some(prepared) = prepared {
            // The journal is replayed event by event rather than collapsed into
            // one aggregate charge. Lump-sum charging was measured against
            // `frame_batch_property`: artifact bytes and reports survive it,
            // but the canonical resource trace does not, and the revision-1
            // contract requires the trace to be exact across the cold,
            // warm, request-cache, and frame-batch-cache paths alike.
            for event in &prepared.normalization_events {
                event.replay(self, request)?;
            }
            for event in &prepared.relationship_events {
                event.replay(self, request)?;
            }
            let target = match self.target {
                RenderTarget::Frame(target) => target,
                RenderTarget::Scene => unreachable!("prepared rendering is frame-scoped"),
            };
            let normalization = prepared
                .normalization_for_target(target)
                .map_err(|_| self.memory_failure(request, "frame_selection", "prepared_target"))?;
            (
                normalization,
                ResolvedRelationships::Replayed(&prepared.relationships),
            )
        } else {
            let normalization = normalize_scene_with_assets(
                scene,
                self.profile.compatibility_revision(),
                self.scene_surface,
                request.format(),
                request.supplemental_assets(),
                matches!(self.target, RenderTarget::Scene)
                    && request.frame_options().enabled()
                    && request.frame_options().name(),
                if matches!(self.target, RenderTarget::Frame(_)) {
                    usize::MAX
                } else {
                    surface_diagnostic_preflight_limit(request)
                },
                |event| self.observe_normalization_event(request, event),
            );
            let normalization = match normalization {
                Ok(normalization) => normalization,
                Err(NormalizationError::Observer(error)) => return Err(error),
                Err(NormalizationError::MemoryUnavailable { source_index }) => {
                    let json_pointer = source_index
                        .and_then(|index| scene::pointer_index("/elements", index).ok())
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
            let relationships = relationships::resolve(
                scene,
                &normalization.elements,
                &normalization.files,
                request.supplemental_assets(),
                |event| self.observe_relationship_event(request, event),
            );
            let relationships = match relationships {
                Ok(relationships) => relationships,
                Err(RelationshipError::Observer(error)) => {
                    return Err(error);
                }
                Err(RelationshipError::MemoryUnavailable) => {
                    return Err(self.memory_failure(
                        request,
                        "relationship_resolution",
                        "relationship_graph",
                    ));
                }
            };
            (normalization, ResolvedRelationships::Live(relationships))
        };
        let (frame_viewport, frame_participating_sources, selected_file_ids) = match self.target {
            RenderTarget::Scene => (None, None, None),
            RenderTarget::Frame(target) => {
                let selection =
                    FrameSelection::compute(&normalization.elements, &relationships.graph, target)
                        .map_err(|_| {
                            self.memory_failure(
                                request,
                                "frame_selection",
                                "participating_elements",
                            )
                        })?;
                let Some(selection) = selection else {
                    self.flush_frame_normalization_diagnostics(
                        request,
                        &[target.source_index],
                        &[],
                    )?;
                    self.scope_frame_diagnostics(&[target.source_index]);
                    self.diagnostic_attribution = DiagnosticAttribution::Target;
                    return Err(self.internal_failure(
                        request.format(),
                        "discovered frame target was not normalized",
                        "/elements",
                    ));
                };
                normalization
                    .elements
                    .retain(|element| selection.retains_element(element));
                normalization
                    .fallbacks
                    .retain(|fallback| selection.retains_fallback(fallback));
                let mut diagnostic_sources = selection.participating.clone();
                diagnostic_sources.extend(
                    normalization
                        .fallbacks
                        .iter()
                        .map(|fallback| fallback.source_index),
                );
                diagnostic_sources.sort_unstable();
                diagnostic_sources.dedup();
                let file_ids = selected_file_ids(&normalization.elements);
                normalization
                    .files
                    .retain(|record| file_ids.binary_search(&record.key).is_ok());
                self.flush_frame_normalization_diagnostics(
                    request,
                    &diagnostic_sources,
                    &file_ids,
                )?;
                self.scope_frame_diagnostics(&diagnostic_sources);
                self.diagnostic_attribution = DiagnosticAttribution::Target;
                let FrameSelection {
                    viewport,
                    participating,
                    ..
                } = selection;
                (Some(viewport), Some(participating), Some(file_ids))
            }
        };
        Ok(ResolutionInputs {
            normalization,
            relationships,
            frame_viewport,
            frame_participating_sources,
            selected_file_ids,
        })
    }

    /// Phase 2: the semantic-accounting census — asset occurrences, removed
    /// relationship cycles, the working-record reservation, and relationship
    /// edges — charged in the canonical trace order.
    fn account_semantic_census(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        inputs: &ResolutionInputs<'_>,
        semantic_working: &mut Vec<SemanticWorkingRecord>,
    ) -> Result<(), RenderError> {
        let ResolutionInputs {
            normalization,
            relationships,
            frame_participating_sources,
            selected_file_ids,
            ..
        } = inputs;
        let occurrence_count = selected_file_ids
            .as_ref()
            .map_or_else(
                || {
                    scene
                        .file_record_count()
                        .checked_add(request.supplemental_assets().len())
                },
                |selected| selected.len().checked_mul(2),
            )
            .ok_or_else(|| self.memory_failure(request, "asset_admission", "occurrence_count"))?;
        let mut asset_occurrence_working = Vec::new();
        asset_occurrence_working
            .try_reserve_exact(occurrence_count)
            .map_err(|_| self.memory_failure(request, "asset_admission", "occurrence_records"))?;
        for index in 0..scene.file_record_count() {
            let Some(record) = scene.file_record_at(index) else {
                continue;
            };
            if selected_file_ids.as_ref().is_some_and(|selected| {
                selected
                    .binary_search_by(|file_id| file_id.as_str().cmp(record.key()))
                    .is_err()
            }) {
                continue;
            }
            let SourceField::Value(data_url) = record.data_url() else {
                continue;
            };
            let decoded_bytes = asset::data_url_decoded_len(data_url);
            let pointer = scene::pointer_child(
                &scene::pointer_child("/files", record.key()).map_err(|_| {
                    self.memory_failure(request, "asset_admission", "scene_asset_pointer")
                })?,
                "dataURL",
            )
            .map_err(|_| self.memory_failure(request, "asset_admission", "scene_asset_pointer"))?;
            let occurrence_bytes = 256_u64.saturating_add(logical_aligned_bytes(
                u64::try_from(if record.key().is_empty() {
                    pointer.len()
                } else {
                    record.key().len()
                })
                .unwrap_or(u64::MAX),
            ));
            self.transact(
                request,
                ResourceContext::new(
                    ResourcePhase::AuthorityReferenceCensus,
                    "asset_occurrence",
                    &pointer,
                )
                .with_local_owner(Some(record.key())),
                ResourceDelta {
                    asset_entries: 1,
                    asset_bytes: decoded_bytes,
                    working_bytes: occurrence_bytes,
                    total_work_units: decoded_bytes,
                    ..ResourceDelta::default()
                },
            )?;
            asset_occurrence_working.push((
                pointer,
                try_owned(record.key()).map_err(|_| {
                    self.memory_failure(request, "asset_admission", "occurrence_owner")
                })?,
                occurrence_bytes,
            ));
        }
        for (file_id, supplemental) in request.supplemental_assets().iter() {
            if selected_file_ids.as_ref().is_some_and(|selected| {
                selected
                    .binary_search_by(|id| id.as_str().cmp(file_id))
                    .is_err()
            }) {
                continue;
            }
            let bytes = u64::try_from(supplemental.bytes().len()).unwrap_or(u64::MAX);
            let occurrence_bytes = 256_u64.saturating_add(logical_aligned_bytes(
                u64::try_from(file_id.len()).unwrap_or(u64::MAX),
            ));
            self.transact(
                request,
                ResourceContext::new(
                    ResourcePhase::AuthorityReferenceCensus,
                    "asset_occurrence",
                    "",
                )
                .with_local_owner(Some(file_id)),
                ResourceDelta {
                    asset_entries: 1,
                    asset_bytes: bytes,
                    working_bytes: occurrence_bytes,
                    total_work_units: bytes.saturating_mul(2),
                    ..ResourceDelta::default()
                },
            )?;
            asset_occurrence_working.push((
                String::new(),
                try_owned(file_id).map_err(|_| {
                    self.memory_failure(request, "asset_admission", "occurrence_owner")
                })?,
                occurrence_bytes,
            ));
        }
        for (pointer, owner, bytes) in &asset_occurrence_working {
            self.release_working(
                request,
                ResourceContext::new(
                    ResourcePhase::AuthorityReferenceCensus,
                    "asset_occurrence",
                    pointer,
                )
                .with_local_owner(Some(owner)),
                *bytes,
            )?;
        }
        for cycle in &relationships.removed_cycles {
            if frame_participating_sources
                .as_ref()
                .is_some_and(|participating| {
                    element_owner_index(&cycle.json_pointer).is_some_and(|source_index| {
                        usize::try_from(source_index)
                            .map(|source_index| participating.binary_search(&source_index).is_err())
                            .unwrap_or(true)
                    })
                })
            {
                continue;
            }
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
                bytes: relationships::edge_working_bytes(),
            });
        }
        Ok(())
    }

    /// Phase 4: post-loop accounting and assembly of the resolved
    /// representation, mapping every `ResolveSceneError` onto the
    /// render-error contract.
    #[allow(clippy::too_many_arguments)]
    fn assemble_resolved_scene(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        outlines: Vec<(NormalizedElement, Option<GeneratedGeometry>)>,
        images: image::ImageResolver,
        mut fallbacks: Vec<SurfaceFallback>,
        chrome: SceneChrome,
        relationships: &RelationshipOutcome,
        semantic_working: &mut Vec<SemanticWorkingRecord>,
    ) -> Result<ResolvedScene, RenderError> {
        let SceneChrome {
            files: normalized_files,
            view_background_color,
            view_background_working_bytes,
            frame_rendering: effective_frame_rendering,
            viewport: frame_viewport,
        } = chrome;
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
        self.preflight_fallback_text(&mut fallbacks, request, semantic_working)?;
        semantic_working.sort_unstable_by_key(|record| record.source_order);
        if outlines.is_empty() && fallbacks.is_empty() {
            match ResolvedScene::empty(&view_background_color, request, frame_viewport) {
                Ok(resolved) => Ok(resolved),
                Err(ResolveSceneError::UnboundedGeometry) => {
                    Err(self.unbounded_geometry_failure(request.format()))
                }
                Err(ResolveSceneError::InvalidColor) => Err(self.internal_failure(
                    request.format(),
                    "normalized scene background color is invalid",
                    "/appState/viewBackgroundColor",
                )),
                Err(
                    ResolveSceneError::Memory
                    | ResolveSceneError::InvalidFallbackFont
                    | ResolveSceneError::InvalidMandatoryNotice,
                ) => Err(self.internal_failure(
                    request.format(),
                    "empty scene resolution reached an impossible failure",
                    "",
                )),
            }
        } else {
            match ResolvedScene::elements(
                outlines,
                images.into_inputs(),
                fallbacks,
                &relationships.graph,
                effective_frame_rendering,
                &view_background_color,
                request,
                frame_viewport,
            ) {
                Ok(Some(resolved)) => Ok(resolved),
                Ok(None) => Err(self.internal_failure(
                    request.format(),
                    "outline semantics are outside the implemented focused case",
                    "/elements",
                )),
                Err(ResolveSceneError::Memory) => {
                    Err(self.memory_failure(request, "semantic_geometry", "outline"))
                }
                Err(ResolveSceneError::InvalidFallbackFont) => Err(self.internal_failure(
                    request.format(),
                    "the sealed fallback font could not be resolved",
                    "/elements",
                )),
                Err(ResolveSceneError::InvalidMandatoryNotice) => Err(self.format_failure(
                    request,
                    format_error_code(request.format()),
                    FormatEncodeError::Encoding(
                        "the mandatory font notice could not be resolved".to_owned(),
                    ),
                )),
                Err(ResolveSceneError::InvalidColor) => Err(self.internal_failure(
                    request.format(),
                    "normalized scene color is invalid",
                    "/elements",
                )),
                Err(ResolveSceneError::UnboundedGeometry) => {
                    Err(self.unbounded_geometry_failure(request.format()))
                }
            }
        }
    }
}
