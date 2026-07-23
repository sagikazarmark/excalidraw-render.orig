mod asset;
mod freedraw;
mod metadata;
mod notices;
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
    DiagnosticSeverity, ErrorCode, ErrorDetails, FiniteValue, Measurement,
    MetadataComponent as ManifestComponent, MetadataOperation, OutputFormat, Recovery,
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
    pending_png_animation_diagnostics: Vec<PendingAnimationDiagnostic>,
}

struct PendingAnimationDiagnostic {
    diagnostic: CompatibilityDiagnostic,
    local_owner: String,
    source_order: u64,
    working_bytes: u64,
}

#[derive(Clone, Copy)]
enum OptionalMetadataComponent {
    Provenance,
    DiagnosticSummary,
    EmbeddedScene,
}

impl OptionalMetadataComponent {
    fn requested(self, selection: metadata::MetadataSelection) -> bool {
        match self {
            Self::Provenance => selection.provenance,
            Self::DiagnosticSummary => selection.diagnostic_summary,
            Self::EmbeddedScene => selection.embedded_scene,
        }
    }

    fn enable(self, selection: &mut metadata::MetadataSelection) {
        match self {
            Self::Provenance => selection.provenance = true,
            Self::DiagnosticSummary => selection.diagnostic_summary = true,
            Self::EmbeddedScene => selection.embedded_scene = true,
        }
    }

    const fn event(self) -> &'static str {
        match self {
            Self::Provenance => "provenance_metadata_omitted",
            Self::DiagnosticSummary => "diagnostic_summary_metadata_omitted",
            Self::EmbeddedScene => "embedded_scene_metadata_omitted",
        }
    }

    const fn manifest_component(self) -> ManifestComponent {
        match self {
            Self::Provenance => ManifestComponent::Provenance,
            Self::DiagnosticSummary => ManifestComponent::DiagnosticSummary,
            Self::EmbeddedScene => ManifestComponent::EmbeddedScene,
        }
    }

    const fn serialization_code(self) -> DiagnosticCode {
        match self {
            Self::Provenance => DiagnosticCode::METADATA_PROVENANCE_SERIALIZATION_FAILED,
            Self::DiagnosticSummary => {
                DiagnosticCode::METADATA_DIAGNOSTIC_SUMMARY_SERIALIZATION_FAILED
            }
            Self::EmbeddedScene => DiagnosticCode::METADATA_SCENE_SERIALIZATION_FAILED,
        }
    }
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
            pending_png_animation_diagnostics: Vec::new(),
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
            pending_png_animation_diagnostics: Vec::new(),
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
                            ResourcePhase::AuthorityReferenceCensus,
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
                                ResourcePhase::AuthorityReferenceCensus,
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
            let asset_authorities =
                relationship_asset_authorities(&relationships.graph).map_err(|_| {
                    self.memory_failure(request, "relationship_resolution", "asset_authorities")
                })?;
            let occurrence_count = scene
                .file_record_count()
                .checked_add(request.supplemental_assets().len())
                .ok_or_else(|| {
                    self.memory_failure(request, "asset_admission", "occurrence_count")
                })?;
            let mut asset_occurrence_working = Vec::new();
            asset_occurrence_working
                .try_reserve_exact(occurrence_count)
                .map_err(|_| {
                    self.memory_failure(request, "asset_admission", "occurrence_records")
                })?;
            for index in 0..scene.file_record_count() {
                let Some(record) = scene.file_record_at(index) else {
                    continue;
                };
                let crate::scene::SourceField::Value(data_url) = record.data_url() else {
                    continue;
                };
                let decoded_bytes = asset::data_url_decoded_len(data_url);
                let pointer = crate::scene::pointer_child(
                    &crate::scene::pointer_child("/files", record.key()).map_err(|_| {
                        self.memory_failure(request, "asset_admission", "scene_asset_pointer")
                    })?,
                    "dataURL",
                )
                .map_err(|_| {
                    self.memory_failure(request, "asset_admission", "scene_asset_pointer")
                })?;
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
            let mut image_assets = std::collections::BTreeMap::new();
            let mut image_fallbacks = Vec::new();
            let mut asset_cache = asset::Cache::default();
            outlines
                .try_reserve_exact(normalization.elements.len())
                .map_err(|_| self.memory_failure(request, "normalization", "outline_list"))?;
            for mut source in normalization.elements {
                debug_assert!(!source.index.is_empty());
                if let NormalizedElementKind::Image(image) = &source.kind {
                    if let Some(file_id) = image.file_id.as_deref() {
                        let authority = asset_authorities
                            .binary_search_by_key(&source.source.source_index(), |(index, _)| {
                                *index
                            })
                            .ok()
                            .and_then(|index| asset_authorities.get(index))
                            .map_or(asset::Authority::Missing, |(_, authority)| *authority);
                        let mut pending_asset_working = Vec::new();
                        pending_asset_working.try_reserve_exact(2).map_err(|_| {
                            self.memory_failure(
                                request,
                                "asset_resolution",
                                "pending_working_records",
                            )
                        })?;
                        let asset_resolution = asset_cache.resolve(
                            scene,
                            request.supplemental_assets(),
                            self.profile.compatibility_revision(),
                            request.format(),
                            request.resource_limits(),
                            file_id,
                            authority,
                            |transaction| match transaction {
                                asset::IntrinsicTransaction::DiscoverNested {
                                    asset_entries,
                                    asset_bytes,
                                    decoded_pixels,
                                } => self.transact(
                                    request,
                                    ResourceContext::new(
                                        ResourcePhase::IntrinsicAssets,
                                        "nested_asset_occurrence",
                                        source.source.pointer(),
                                    )
                                    .with_local_owner(Some(file_id)),
                                    ResourceDelta {
                                        asset_entries,
                                        asset_bytes,
                                        decoded_pixels,
                                        total_work_units: asset_bytes,
                                        ..ResourceDelta::default()
                                    },
                                ),
                                asset::IntrinsicTransaction::Acquire {
                                    event,
                                    identity,
                                    persistent_working_bytes,
                                    temporary_working_bytes,
                                    decoded_pixels,
                                    total_work_units,
                                } => {
                                    let identity =
                                        semantic_identity_hex(&identity).map_err(|_| {
                                            self.memory_failure(
                                                request,
                                                "asset_resolution",
                                                "semantic_identity",
                                            )
                                        })?;
                                    let record = (persistent_working_bytes != 0)
                                        .then(|| {
                                            Ok::<_, RenderError>(SemanticWorkingRecord {
                                                source_order: source.source_order,
                                                event,
                                                element_id: None,
                                                local_owner: None,
                                                json_pointer: try_owned(source.source.pointer())
                                                    .map_err(|_| {
                                                        self.memory_failure(
                                                            request,
                                                            "asset_resolution",
                                                            "semantic_working_pointer",
                                                        )
                                                    })?,
                                                bytes: persistent_working_bytes,
                                            })
                                        })
                                        .transpose()?;
                                    self.transact(
                                        request,
                                        ResourceContext::new(
                                            ResourcePhase::IntrinsicAssets,
                                            event,
                                            source.source.pointer(),
                                        )
                                        .with_semantic_identity(&identity)
                                        .with_sharing_key(event),
                                        ResourceDelta {
                                            decoded_pixels,
                                            working_bytes: persistent_working_bytes
                                                .saturating_add(temporary_working_bytes),
                                            total_work_units,
                                            ..ResourceDelta::default()
                                        },
                                    )?;
                                    if let Some(record) = record {
                                        pending_asset_working.push(record);
                                    }
                                    Ok(())
                                }
                                asset::IntrinsicTransaction::ReleaseTemporary {
                                    event,
                                    identity,
                                    bytes,
                                } => {
                                    let identity =
                                        semantic_identity_hex(&identity).map_err(|_| {
                                            self.memory_failure(
                                                request,
                                                "asset_resolution",
                                                "semantic_identity",
                                            )
                                        })?;
                                    self.release_working(
                                        request,
                                        ResourceContext::new(
                                            ResourcePhase::IntrinsicAssets,
                                            event,
                                            source.source.pointer(),
                                        )
                                        .with_semantic_identity(&identity),
                                        bytes,
                                    )
                                }
                                asset::IntrinsicTransaction::Abort {
                                    event,
                                    identity,
                                    bytes,
                                } => {
                                    let identity =
                                        semantic_identity_hex(&identity).map_err(|_| {
                                            self.memory_failure(
                                                request,
                                                "asset_resolution",
                                                "semantic_identity",
                                            )
                                        })?;
                                    self.release_working(
                                        request,
                                        ResourceContext::new(
                                            ResourcePhase::IntrinsicAssets,
                                            event,
                                            source.source.pointer(),
                                        )
                                        .with_semantic_identity(&identity),
                                        bytes,
                                    )?;
                                    pending_asset_working.retain(|record| record.event != event);
                                    Ok(())
                                }
                            },
                        );
                        let asset_resolution = match asset_resolution {
                            Ok(resolution) => Ok(resolution),
                            Err(asset::ResolutionFailure::Asset(error)) => Err(error),
                            Err(asset::ResolutionFailure::Transaction(error)) => return Err(error),
                        };
                        semantic_working.append(&mut pending_asset_working);
                        let asset_failure = asset_resolution.as_ref().err().copied();
                        if let Ok(resolution) = asset_resolution {
                            let animated = resolution.animated;
                            let media_type = resolution.media_kind.media_type();
                            let svg_diagnostic_location = if resolution.svg_removals.is_empty()
                                && resolution.svg_failures.is_empty()
                            {
                                None
                            } else {
                                Some(
                                    asset_failure_location(
                                        scene,
                                        request.supplemental_assets(),
                                        file_id,
                                        authority,
                                    )
                                    .map_err(|_| {
                                        self.memory_failure(
                                            request,
                                            "compatibility_report",
                                            "unsafe_svg_location",
                                        )
                                    })?,
                                )
                            };
                            for removal in &resolution.svg_removals {
                                let (json_pointer, source_order, source_position, local_owner) =
                                    svg_diagnostic_location.as_ref().ok_or_else(|| {
                                        self.internal_failure(
                                            request.format(),
                                            "unsafe SVG diagnostic lost its source location",
                                            "/files",
                                        )
                                    })?;
                                let diagnostic = unsafe_svg_removal_diagnostic(
                                    request.format(),
                                    file_id,
                                    json_pointer,
                                    removal,
                                )
                                .map_err(|_| {
                                    self.memory_failure(
                                        request,
                                        "compatibility_report",
                                        "unsafe_svg_removal",
                                    )
                                })?;
                                self.retain_diagnostic_with_owner(
                                    request,
                                    ResourcePhase::SharedSemanticResolution,
                                    "unsafe_svg_removal",
                                    diagnostic,
                                    DiagnosticOwnership {
                                        local_owner: Some(local_owner),
                                        source_order: Some(*source_order),
                                        source_position: Some(
                                            source_position.saturating_add(removal.node_ordinal),
                                        ),
                                    },
                                )?;
                            }
                            for failure in &resolution.svg_failures {
                                let (json_pointer, source_order, source_position, local_owner) =
                                    svg_diagnostic_location.as_ref().ok_or_else(|| {
                                        self.internal_failure(
                                            request.format(),
                                            "nested SVG failure lost its source location",
                                            "/files",
                                        )
                                    })?;
                                let diagnostic = nested_image_failure_diagnostic(
                                    failure,
                                    request.format(),
                                    file_id,
                                    json_pointer,
                                    request.resource_limits(),
                                )
                                .map_err(|_| {
                                    self.memory_failure(
                                        request,
                                        "compatibility_report",
                                        "nested_image_failure",
                                    )
                                })?;
                                self.retain_diagnostic_with_owner(
                                    request,
                                    ResourcePhase::SharedSemanticResolution,
                                    "nested_image_failure",
                                    diagnostic,
                                    DiagnosticOwnership {
                                        local_owner: Some(local_owner),
                                        source_order: Some(*source_order),
                                        source_position: Some(source_position.saturating_add(
                                            failure.path.first().map_or(0, |slot| slot.0),
                                        )),
                                    },
                                )?;
                            }
                            let asset = resolution.asset;
                            for (claim, field) in [
                                (resolution.claims.mime_type.as_deref(), "mimeType"),
                                (resolution.claims.data_url_media_type.as_deref(), "dataURL"),
                            ] {
                                let Some(claim) = claim.filter(|claim| {
                                    !claim
                                        .split(';')
                                        .next()
                                        .unwrap_or_default()
                                        .trim()
                                        .eq_ignore_ascii_case(media_type)
                                }) else {
                                    continue;
                                };
                                let (json_pointer, source_order, source_position, local_owner) =
                                    asset_claim_location(
                                        scene,
                                        request.supplemental_assets(),
                                        file_id,
                                        authority,
                                        field,
                                    )
                                    .map_err(|_| {
                                        self.memory_failure(
                                            request,
                                            "compatibility_report",
                                            "asset_pointer",
                                        )
                                    })?;
                                let diagnostic = crate::scene_surface::asset_media_type_diagnostic(
                                    request.format(),
                                    file_id,
                                    &json_pointer,
                                    Some(claim),
                                    Some(media_type),
                                )
                                .map_err(|_| {
                                    self.memory_failure(
                                        request,
                                        "compatibility_report",
                                        "asset_media_type_mismatch",
                                    )
                                })?;
                                self.retain_diagnostic_with_owner(
                                    request,
                                    ResourcePhase::SharedSemanticResolution,
                                    "asset_media_type_mismatch",
                                    diagnostic,
                                    DiagnosticOwnership {
                                        local_owner: Some(&local_owner),
                                        source_order: Some(source_order),
                                        source_position: Some(source_position),
                                    },
                                )?;
                            }
                            if animated && request.format() != OutputFormat::Svg {
                                let file_id_pointer =
                                    crate::scene::pointer_child(source.source.pointer(), "fileId")
                                        .map_err(|_| {
                                            self.memory_failure(
                                                request,
                                                "compatibility_report",
                                                "image_animation_pointer",
                                            )
                                        })?;
                                if request.format() == OutputFormat::Png {
                                    let pointer_bytes =
                                        u64::try_from(file_id_pointer.len()).unwrap_or(u64::MAX);
                                    let owner_bytes =
                                        u64::try_from(source.source.id().len()).unwrap_or(u64::MAX);
                                    let working_bytes = 512_u64
                                        .saturating_add(logical_aligned_bytes(pointer_bytes))
                                        .saturating_add(logical_aligned_bytes(owner_bytes));
                                    self.acquire_working(
                                        request,
                                        ResourceContext::new(
                                            ResourcePhase::SharedSemanticResolution,
                                            "pending_image_animation_flattened",
                                            &file_id_pointer,
                                        )
                                        .with_element_id(Some(source.source.id()))
                                        .with_local_owner(Some(source.source.id())),
                                        working_bytes,
                                    )?;
                                    self.pending_png_animation_diagnostics
                                        .try_reserve(1)
                                        .map_err(|_| {
                                            self.memory_failure(
                                                request,
                                                "compatibility_report",
                                                "pending_image_animation_flattened",
                                            )
                                        })?;
                                    let local_owner =
                                        try_owned(source.source.id()).map_err(|_| {
                                            self.memory_failure(
                                                request,
                                                "compatibility_report",
                                                "pending_image_animation_owner",
                                            )
                                        })?;
                                    let diagnostic = image_animation_flattened_diagnostic(
                                        request.format(),
                                        file_id,
                                        source.source.id(),
                                        &file_id_pointer,
                                        media_type,
                                    )
                                    .map_err(|_| {
                                        self.memory_failure(
                                            request,
                                            "compatibility_report",
                                            "image_animation_flattened",
                                        )
                                    })?;
                                    self.pending_png_animation_diagnostics.push(
                                        PendingAnimationDiagnostic {
                                            diagnostic,
                                            local_owner,
                                            source_order: source.source_order,
                                            working_bytes,
                                        },
                                    );
                                } else {
                                    let diagnostic = image_animation_flattened_diagnostic(
                                        request.format(),
                                        file_id,
                                        source.source.id(),
                                        &file_id_pointer,
                                        media_type,
                                    )
                                    .map_err(|_| {
                                        self.memory_failure(
                                            request,
                                            "compatibility_report",
                                            "image_animation_flattened",
                                        )
                                    })?;
                                    self.retain_diagnostic_with_owner(
                                        request,
                                        ResourcePhase::SharedSemanticResolution,
                                        "image_animation_flattened",
                                        diagnostic,
                                        DiagnosticOwnership {
                                            local_owner: Some(source.source.id()),
                                            source_order: Some(source.source_order),
                                            source_position: Some(u64::MAX),
                                        },
                                    )?;
                                }
                            }
                            image_assets.insert(source.source.source_index(), asset);
                            semantic_working.push(SemanticWorkingRecord {
                                source_order: source.source_order,
                                event: "resolved_image",
                                element_id: Some(try_owned(source.source.id()).map_err(|_| {
                                    self.memory_failure(
                                        request,
                                        "normalization",
                                        "working_record_identity",
                                    )
                                })?),
                                local_owner: Some(try_owned(source.source.id()).map_err(|_| {
                                    self.memory_failure(
                                        request,
                                        "normalization",
                                        "working_record_owner",
                                    )
                                })?),
                                json_pointer: try_owned(source.source.pointer()).map_err(|_| {
                                    self.memory_failure(
                                        request,
                                        "normalization",
                                        "working_record_pointer",
                                    )
                                })?,
                                bytes: source.normalized_working_bytes,
                            });
                            outlines.push((source, None));
                            continue;
                        }
                        if let Some(failure) = asset_failure {
                            if matches!(failure, asset::ResolveError::Memory) {
                                return Err(self.memory_failure(
                                    request,
                                    "asset_resolution",
                                    "raster_asset",
                                ));
                            }
                            if !matches!(failure, asset::ResolveError::Missing)
                                && !self.report.diagnostics.iter().any(|diagnostic| {
                                    matches!(
                                        diagnostic.details(),
                                        Some(ErrorDetails::Asset { asset_id, .. })
                                            if asset_id == file_id
                                    ) && diagnostic_replaces_element(diagnostic)
                                })
                            {
                                let (json_pointer, source_order, source_position, local_owner) =
                                    asset_failure_location(
                                        scene,
                                        request.supplemental_assets(),
                                        file_id,
                                        authority,
                                    )
                                    .map_err(|_| {
                                        self.memory_failure(
                                            request,
                                            "compatibility_report",
                                            "asset_failure_pointer",
                                        )
                                    })?;
                                let diagnostic = image_asset_failure_diagnostic(
                                    failure,
                                    request.format(),
                                    file_id,
                                    source.source.id(),
                                    &json_pointer,
                                    image.status,
                                    request.resource_limits(),
                                )
                                .map_err(|_| {
                                    self.memory_failure(
                                        request,
                                        "compatibility_report",
                                        "asset_failure",
                                    )
                                })?;
                                self.retain_diagnostic_with_owner(
                                    request,
                                    ResourcePhase::SharedSemanticResolution,
                                    "asset_failure",
                                    diagnostic,
                                    DiagnosticOwnership {
                                        local_owner: Some(&local_owner),
                                        source_order: Some(source_order),
                                        source_position: Some(source_position),
                                    },
                                )?;
                            }
                        }
                    }
                    if request.format() == OutputFormat::Png
                        && self.report.diagnostics.iter().any(|diagnostic| {
                            diagnostic_uses_non_generic_recovery(&source, diagnostic)
                        })
                    {
                        image_fallbacks.try_reserve(1).map_err(|_| {
                            self.memory_failure(request, "asset_resolution", "image_fallbacks")
                        })?;
                        image_fallbacks.push((source.source.source_index(), image.status));
                        semantic_working.push(SemanticWorkingRecord {
                            source_order: source.source_order,
                            event: "resolved_image_fallback",
                            element_id: Some(try_owned(source.source.id()).map_err(|_| {
                                self.memory_failure(
                                    request,
                                    "normalization",
                                    "working_record_identity",
                                )
                            })?),
                            local_owner: Some(try_owned(source.source.id()).map_err(|_| {
                                self.memory_failure(
                                    request,
                                    "normalization",
                                    "working_record_owner",
                                )
                            })?),
                            json_pointer: try_owned(source.source.pointer()).map_err(|_| {
                                self.memory_failure(
                                    request,
                                    "normalization",
                                    "working_record_pointer",
                                )
                            })?,
                            bytes: source.normalized_working_bytes,
                        });
                        outlines.push((source, None));
                        continue;
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
                        diagnostic_uses_non_generic_recovery(&source, diagnostic)
                            || diagnostic.element_id() == Some(source.source.id())
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
                    Err(
                        ResolveSceneError::Memory
                        | ResolveSceneError::InvalidFallbackFont
                        | ResolveSceneError::InvalidMandatoryNotice,
                    ) => {
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
                    ResolvedImageInputs {
                        assets: image_assets,
                        fallbacks: image_fallbacks,
                    },
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
                    Err(ResolveSceneError::InvalidMandatoryNotice) => {
                        return Err(self.format_failure(
                            request,
                            format_error_code(request.format()),
                            FormatEncodeError::Encoding(
                                "the mandatory font notice could not be resolved".to_owned(),
                            ),
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
        let bytes = self.encode(resolved, scene, request, &semantic_working)?;

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
        scene: &SceneDocument,
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
                let mut plan = SvgPlan::new(resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_SVG_FAILED, error)
                })?;
                self.release_semantic_working(request, semantic_working, false)?;
                let workspace_bytes = 256 * 1_024;
                let workspace = ResourceContext::new(
                    ResourcePhase::EncodingAndOutcomeAssembly,
                    "svg_serializer_workspace",
                    "",
                )
                .with_record(ResourceRecordKind::SvgSerializerWorkspace);
                self.acquire_working(request, workspace, workspace_bytes)?;
                let mandatory_notices = plan.take_mandatory_notices();
                let (metadata, measured) = self.select_artifact_metadata(
                    scene,
                    request,
                    mandatory_notices.as_ref(),
                    ErrorCode::ENCODE_SVG_FAILED,
                    0,
                    |metadata| {
                        plan.set_metadata(metadata.clone());
                        plan.encode()
                    },
                )?;
                plan.set_metadata(metadata);
                self.emit_premeasured_output(
                    request,
                    ErrorCode::ENCODE_SVG_FAILED,
                    0,
                    measured,
                    || plan.encode(),
                )?;
                self.release_semantic_working(request, semantic_working, true)?;
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
                self.release_semantic_working(request, semantic_working, false)?;
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
                let paint_mask_bytes = plan
                    .maximum_paint_mask_surface_area()
                    .saturating_add(u64::from(plan.has_paint_masks()) * 256);
                let paint_mask_surface = ResourceContext::new(
                    ResourcePhase::ProjectionAndFallback,
                    "frame_clip_mask_surface",
                    "",
                )
                .with_record(ResourceRecordKind::RasterMaskSurface);
                if paint_mask_bytes != 0 {
                    self.acquire_working(request, paint_mask_surface, paint_mask_bytes)?;
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
                    let mut paint_mask_pixels = None;
                    if let Some(mask_pixels) =
                        plan.paint_mask_surface_area(index).ok_or_else(|| {
                            self.internal_failure(
                                request.format(),
                                "PNG paint mask source is missing",
                                "/elements",
                            )
                        })?
                    {
                        let surface_limit = request.resource_limits().max_raster_surface_pixels();
                        if mask_pixels > surface_limit {
                            let diagnostic = ElementLocalCrossing {
                                code: DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED,
                                field: "max_raster_surface_pixels",
                                configured: ResourceValue::Integer(surface_limit),
                                value_kind: ValueKind::Pixels,
                                attempted: ResourceValue::Integer(mask_pixels),
                                pointer_field: None,
                                message: "masked element omitted because its paint mask exceeds its raster surface limit",
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
                            self.discard_pending_png_animation_diagnostic(request, element_id)?;
                            if !plan.omit_element(index) {
                                return Err(self.internal_failure(
                                    request.format(),
                                    "PNG omitted masked operation is missing",
                                    "/elements",
                                ));
                            }
                            continue;
                        }
                        paint_mask_pixels = Some(mask_pixels);
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
                            self.discard_pending_png_animation_diagnostic(request, element_id)?;
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
                    let image_projection = plan.image_projection_area(index).ok_or_else(|| {
                        self.internal_failure(
                            request.format(),
                            "PNG image projection source is missing",
                            "/elements",
                        )
                    })?;
                    let (projection_work, composite_work, projection_event) =
                        if let Some(area) = image_projection {
                            (
                                area.saturating_mul(12),
                                area.saturating_mul(4),
                                "image_bilinear_projection_samples",
                            )
                        } else {
                            (
                                plan.vector_coverage_area(index)
                                    .ok_or_else(|| {
                                        self.internal_failure(
                                            request.format(),
                                            "PNG vector coverage source is missing",
                                            "/elements",
                                        )
                                    })?
                                    .saturating_mul(16),
                                0,
                                "vector_coverage_samples",
                            )
                        };
                    let mask_work = paint_mask_pixels.unwrap_or(0).saturating_mul(8);
                    let raster_work = projection_work
                        .saturating_add(mask_work)
                        .saturating_add(composite_work);
                    let context = ResourceContext::new(
                        ResourcePhase::ProjectionAndFallback,
                        projection_event,
                        json_pointer,
                    )
                    .with_element_id(element_id)
                    .with_local_owner(element_id);
                    let combined_delta = ResourceDelta {
                        rasterized_pixels: paint_mask_pixels.unwrap_or(0),
                        total_work_units: raster_work,
                        ..ResourceDelta::default()
                    };
                    if self.meter.would_cross_fatal(combined_delta) {
                        self.transact(request, context, combined_delta)?;
                        return Err(self.internal_failure(
                            request.format(),
                            "fatal PNG element raster preflight committed unexpectedly",
                            "/elements",
                        ));
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
                        .saturating_add(raster_work);
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
                        self.discard_pending_png_animation_diagnostic(request, element_id)?;
                        if !plan.omit_element(index) {
                            return Err(self.internal_failure(
                                request.format(),
                                "PNG omitted source operation is missing",
                                "/elements",
                            ));
                        }
                    } else {
                        if let Some(mask_pixels) = paint_mask_pixels {
                            self.transact(
                                request,
                                ResourceContext::new(
                                    ResourcePhase::ProjectionAndFallback,
                                    "paint_mask_samples",
                                    json_pointer,
                                )
                                .with_element_id(element_id)
                                .with_local_owner(element_id)
                                .with_record(ResourceRecordKind::RasterMaskSurface),
                                ResourceDelta {
                                    rasterized_pixels: mask_pixels,
                                    total_work_units: mask_work,
                                    ..ResourceDelta::default()
                                },
                            )?;
                        }
                        self.charge_work(request, context, projection_work)?;
                        if composite_work != 0 {
                            self.charge_work(
                                request,
                                ResourceContext::new(
                                    ResourcePhase::ProjectionAndFallback,
                                    "image_composite_samples",
                                    json_pointer,
                                )
                                .with_element_id(element_id)
                                .with_local_owner(element_id),
                                composite_work,
                            )?;
                        }
                    }
                }

                self.flush_pending_png_animation_diagnostics(request)?;

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
                let mandatory_notices = plan.mandatory_notices().map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_PNG_FAILED, error)
                })?;
                let (metadata, measured) = self.select_artifact_metadata(
                    scene,
                    request,
                    mandatory_notices.as_ref(),
                    ErrorCode::ENCODE_PNG_FAILED,
                    rgba_bytes,
                    |metadata| {
                        plan.set_metadata(metadata.clone());
                        plan.encode()
                    },
                )?;
                plan.set_metadata(metadata);
                self.emit_premeasured_output(
                    request,
                    ErrorCode::ENCODE_PNG_FAILED,
                    rgba_bytes,
                    measured,
                    || plan.encode(),
                )?;
                self.release_semantic_working(request, semantic_working, true)?;
                if paint_mask_bytes != 0 {
                    self.release_working(request, paint_mask_surface, paint_mask_bytes)?;
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
                let mut plan = PdfPlan::new(resolved, request).map_err(|error| {
                    self.format_failure(request, ErrorCode::ENCODE_PDF_FAILED, error)
                })?;
                let fallback_surface_working =
                    self.preflight_pdf_raster_fallbacks(request, &plan)?;
                self.release_semantic_working(request, semantic_working, false)?;
                let image_resource_pixels =
                    plan.image_resource_pixel_counts().map_err(|error| {
                        self.format_failure(request, ErrorCode::ENCODE_PDF_FAILED, error)
                    })?;
                let mut image_resource_working = Vec::new();
                image_resource_working
                    .try_reserve_exact(image_resource_pixels.len())
                    .map_err(|_| {
                        self.memory_failure(request, "format_planning", "pdf_image_resources")
                    })?;
                for pixels in image_resource_pixels {
                    let rgb_bytes = 256_u64.saturating_add(pixels.saturating_mul(3));
                    let alpha_bytes = 256_u64.saturating_add(pixels);
                    self.acquire_working(
                        request,
                        ResourceContext::new(
                            ResourcePhase::FormatPlanning,
                            "pdf_image_rgb_format_resource",
                            "",
                        )
                        .with_record(ResourceRecordKind::PdfImageRgbFormatResource),
                        rgb_bytes,
                    )?;
                    self.acquire_working(
                        request,
                        ResourceContext::new(
                            ResourcePhase::FormatPlanning,
                            "pdf_image_alpha_format_resource",
                            "",
                        )
                        .with_record(ResourceRecordKind::PdfImageAlphaFormatResource),
                        alpha_bytes,
                    )?;
                    image_resource_working.push((rgb_bytes, alpha_bytes));
                }
                let workspace = ResourceContext::new(
                    ResourcePhase::EncodingAndOutcomeAssembly,
                    "pdf_stream_workspace",
                    "",
                )
                .with_record(ResourceRecordKind::PdfStreamWorkspace);
                self.acquire_working(request, workspace, workspace_bytes)?;
                let mandatory_notices = plan.take_mandatory_notices();
                let (metadata, measured) = self.select_artifact_metadata(
                    scene,
                    request,
                    mandatory_notices.as_ref(),
                    ErrorCode::ENCODE_PDF_FAILED,
                    0,
                    |metadata| {
                        plan.set_metadata(metadata.clone());
                        plan.encode()
                    },
                )?;
                plan.set_metadata(metadata);
                self.emit_premeasured_output(
                    request,
                    ErrorCode::ENCODE_PDF_FAILED,
                    0,
                    measured,
                    || plan.encode(),
                )?;
                self.release_semantic_working(request, semantic_working, true)?;
                for (rgb_bytes, alpha_bytes) in image_resource_working {
                    self.release_working(
                        request,
                        ResourceContext::new(
                            ResourcePhase::EncodingAndOutcomeAssembly,
                            "pdf_image_rgb_format_resource",
                            "",
                        )
                        .with_record(ResourceRecordKind::PdfImageRgbFormatResource),
                        rgb_bytes,
                    )?;
                    self.release_working(
                        request,
                        ResourceContext::new(
                            ResourcePhase::EncodingAndOutcomeAssembly,
                            "pdf_image_alpha_format_resource",
                            "",
                        )
                        .with_record(ResourceRecordKind::PdfImageAlphaFormatResource),
                        alpha_bytes,
                    )?;
                }
                for (closure, bytes) in plan.raster_closures().iter().zip(fallback_surface_working)
                {
                    self.release_working(
                        request,
                        ResourceContext::new(
                            ResourcePhase::EncodingAndOutcomeAssembly,
                            "pdf_raster_fallback_surface",
                            &closure.json_pointer,
                        )
                        .with_element_id(Some(&closure.element_id))
                        .with_local_owner(Some(&closure.element_id))
                        .with_record(ResourceRecordKind::RasterColorSurface),
                        bytes,
                    )?;
                }
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

    #[allow(clippy::too_many_arguments)]
    fn select_artifact_metadata(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        mandatory_notices: Option<&notices::MandatoryNotices>,
        error_code: ErrorCode,
        codec_emission_work: u64,
        mut measure: impl FnMut(&metadata::ArtifactMetadata) -> Result<Vec<u8>, FormatEncodeError>,
    ) -> Result<(metadata::ArtifactMetadata, Vec<u8>), RenderError> {
        let snapshot = self.report.clone();
        let requested = metadata::MetadataSelection::requested(request);
        let mut accepted = metadata::MetadataSelection::default();
        let baseline = metadata::ArtifactMetadata::build(
            scene,
            self.profile,
            &snapshot,
            mandatory_notices,
            accepted,
        )
        .map_err(|error| self.format_failure(request, error_code, error))?;
        let baseline_artifact =
            measure(&baseline).map_err(|error| self.format_failure(request, error_code, error))?;
        let baseline_bytes = u64::try_from(baseline_artifact.len()).unwrap_or(u64::MAX);
        if self
            .meter
            .optional_metadata_crossing(artifact_retention_delta(
                baseline_bytes,
                codec_emission_work,
            ))
            .is_some()
        {
            if mandatory_notices.is_some() {
                return Err(self.format_failure(
                    request,
                    error_code,
                    FormatEncodeError::Encoding(
                        "mandatory notice cannot be retained in the artifact".to_owned(),
                    ),
                ));
            }
            return Ok((baseline, baseline_artifact));
        }
        let mut accepted_metadata = baseline;
        let mut accepted_artifact = baseline_artifact;

        for component in [
            OptionalMetadataComponent::Provenance,
            OptionalMetadataComponent::DiagnosticSummary,
            OptionalMetadataComponent::EmbeddedScene,
        ] {
            if !component.requested(requested) {
                continue;
            }
            let mut candidate_selection = accepted;
            component.enable(&mut candidate_selection);
            let candidate = match metadata::ArtifactMetadata::build(
                scene,
                self.profile,
                &snapshot,
                mandatory_notices,
                candidate_selection,
            ) {
                Ok(candidate) => candidate,
                Err(error @ FormatEncodeError::MemoryUnavailable { .. }) => {
                    return Err(self.format_failure(request, error_code, error));
                }
                Err(FormatEncodeError::Encoding(_)) => {
                    self.retain_diagnostic(
                        request,
                        ResourcePhase::Metadata,
                        component.event(),
                        metadata_component_failure_diagnostic(
                            request.format(),
                            component,
                            MetadataOperation::Serialization,
                        ),
                    )?;
                    continue;
                }
            };
            let compression_workspace = u64::from(matches!(
                component,
                OptionalMetadataComponent::EmbeddedScene
            )) * 1_048_576;
            let scratch_bytes = candidate
                .scratch_bytes()
                .saturating_sub(accepted_metadata.scratch_bytes())
                .saturating_add(compression_workspace);
            let generation_work = candidate
                .generation_work()
                .saturating_sub(accepted_metadata.generation_work());
            let generation_delta = ResourceDelta {
                working_bytes: scratch_bytes,
                total_work_units: generation_work,
                ..ResourceDelta::default()
            };
            if let Some(crossing) = self.meter.optional_metadata_crossing(generation_delta) {
                self.retain_diagnostic(
                    request,
                    ResourcePhase::Metadata,
                    component.event(),
                    optional_metadata_omitted_diagnostic(request.format(), crossing),
                )?;
                continue;
            }
            let generation_context =
                ResourceContext::new(ResourcePhase::Metadata, component.event(), "");
            self.transact(request, generation_context, generation_delta)?;
            let measured = measure(&candidate);
            self.release_working(request, generation_context, scratch_bytes)?;
            let candidate_artifact = match measured {
                Ok(candidate_artifact) => candidate_artifact,
                Err(error @ FormatEncodeError::MemoryUnavailable { .. }) => {
                    return Err(self.format_failure(request, error_code, error));
                }
                Err(FormatEncodeError::Encoding(_)) => {
                    self.retain_diagnostic(
                        request,
                        ResourcePhase::Metadata,
                        component.event(),
                        metadata_component_failure_diagnostic(
                            request.format(),
                            component,
                            MetadataOperation::Embedding,
                        ),
                    )?;
                    continue;
                }
            };
            let output_bytes = u64::try_from(candidate_artifact.len()).unwrap_or(u64::MAX);
            if let Some(crossing) = self
                .meter
                .optional_metadata_crossing(artifact_retention_delta(
                    output_bytes,
                    codec_emission_work,
                ))
            {
                self.retain_diagnostic(
                    request,
                    ResourcePhase::Metadata,
                    component.event(),
                    optional_metadata_omitted_diagnostic(request.format(), crossing),
                )?;
            } else {
                accepted = candidate_selection;
                accepted_metadata = candidate;
                accepted_artifact = candidate_artifact;
            }
        }

        Ok((accepted_metadata, accepted_artifact))
    }

    fn account_fallback_semantics(
        &mut self,
        resolved: &mut ResolvedScene,
        request: &RenderRequest,
        semantic_working: &mut Vec<SemanticWorkingRecord>,
    ) -> Result<(), RenderError> {
        for element in &mut resolved.elements {
            if let ResolvedElement::Outline(outline) = element {
                self.account_embeddable_label(outline, request)?;
            }
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
        let has_official_text = resolved.elements.iter().any(|element| match element {
            ResolvedElement::Text(_) => true,
            ResolvedElement::Outline(outline) => outline.generated_label.is_some(),
            _ => false,
        });
        if !has_fallback_label {
            resolved.fallback_font_woff2 = None;
            resolved.fallback_font_sfnt = None;
        }
        if !has_official_text {
            resolved.official_fonts.clear();
        }
        resolved.mandatory_notices = if has_fallback_label || has_official_text {
            Some(
                text::mandatory_notices_json(has_fallback_label, &resolved.official_fonts)
                    .map_err(|error| match error {
                        text::LabelError::Memory => self.memory_failure(
                            request,
                            "artifact_manifest",
                            "font_notice_manifest",
                        ),
                        text::LabelError::InvalidFont => self.format_failure(
                            request,
                            format_error_code(request.format()),
                            FormatEncodeError::Encoding(
                                "the mandatory font notice could not be resolved".to_owned(),
                            ),
                        ),
                    })?,
            )
        } else {
            None
        };
        semantic_working.sort_unstable_by_key(|record| record.source_order);
        Ok(())
    }

    fn preflight_pdf_raster_fallbacks(
        &mut self,
        request: &RenderRequest,
        plan: &PdfPlan,
    ) -> Result<Vec<u64>, RenderError> {
        for closure in plan.raster_closures() {
            if closure.unisolatable {
                return Err(self.pdf_raster_fallback_failure(
                    request,
                    closure,
                    ErrorCode::PDF_RASTER_FALLBACK_UNISOLATABLE,
                    "PDF raster fallback requires content outside an isolated nested-SVG closure",
                ));
            }
            if request.pdf_options().is_some_and(|options| {
                options.raster_fallback() == crate::PdfRasterFallback::Forbid
            }) {
                return Err(self.pdf_raster_fallback_failure(
                    request,
                    closure,
                    ErrorCode::PDF_RASTER_FALLBACK_FORBIDDEN,
                    "PDF raster fallback is forbidden by the render request",
                ));
            }
        }
        let mut surface_working = Vec::new();
        surface_working
            .try_reserve_exact(plan.raster_closures().len())
            .map_err(|_| {
                self.memory_failure(request, "format_planning", "pdf_raster_fallback_surfaces")
            })?;
        for closure in plan.raster_closures() {
            let pixels = closure.pixel_width.saturating_mul(closure.pixel_height);
            let limits = request.resource_limits();
            if closure.pixel_width > u64::from(limits.max_raster_dimension())
                || closure.pixel_height > u64::from(limits.max_raster_dimension())
            {
                return Err(self.pdf_raster_surface_failure(
                    request,
                    closure,
                    "max_raster_dimension",
                    u64::from(limits.max_raster_dimension()),
                    closure.pixel_width.max(closure.pixel_height),
                    ValueKind::Dimension,
                ));
            }
            if pixels > limits.max_raster_surface_pixels() {
                return Err(self.pdf_raster_surface_failure(
                    request,
                    closure,
                    "max_raster_surface_pixels",
                    limits.max_raster_surface_pixels(),
                    pixels,
                    ValueKind::Pixels,
                ));
            }
            self.transact(
                request,
                ResourceContext::new(
                    ResourcePhase::ProjectionAndFallback,
                    "pdf_raster_fallback",
                    &closure.json_pointer,
                )
                .with_element_id(Some(&closure.element_id))
                .with_local_owner(Some(&closure.element_id)),
                ResourceDelta {
                    rasterized_pixels: pixels,
                    total_work_units: pixels,
                    ..ResourceDelta::default()
                },
            )?;
            let bytes = 256_u64.saturating_add(pixels.saturating_mul(4));
            self.acquire_working(
                request,
                ResourceContext::new(
                    ResourcePhase::ProjectionAndFallback,
                    "pdf_raster_fallback_surface",
                    &closure.json_pointer,
                )
                .with_element_id(Some(&closure.element_id))
                .with_local_owner(Some(&closure.element_id))
                .with_record(ResourceRecordKind::RasterColorSurface),
                bytes,
            )?;
            surface_working.push(bytes);
        }
        for closure in plan.raster_closures() {
            let diagnostic = pdf_raster_fallback_diagnostic(closure).map_err(|_| {
                self.memory_failure(request, "compatibility_report", "pdf_raster_fallback")
            })?;
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::ProjectionAndFallback,
                "pdf_raster_fallback",
                diagnostic,
                DiagnosticOwnership {
                    local_owner: Some(&closure.element_id),
                    source_order: Some(closure.source_order),
                    source_position: Some(closure.source_position),
                },
            )?;
        }
        Ok(surface_working)
    }

    fn pdf_raster_surface_failure(
        &mut self,
        request: &RenderRequest,
        closure: &pdf::PdfRasterClosure,
        field: &'static str,
        configured: u64,
        attempted: u64,
        value_kind: ValueKind,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::ResourceLimit,
            code: ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED,
            message: Box::from("PDF raster fallback surface exceeds its configured limit"),
            format: request.format(),
            element_id: Some(closure.element_id.clone()),
            json_pointer: closure.json_pointer.clone().into_boxed_str(),
            details: Some(Box::new(ErrorDetails::ResourceLimit {
                field,
                configured: ResourceValue::Integer(configured),
                value_kind,
                measurement: Measurement::Attempted {
                    value: ResourceValue::Integer(attempted),
                },
                recovery: None,
            })),
            report: self.take_report(),
        }
    }

    fn pdf_raster_fallback_failure(
        &mut self,
        request: &RenderRequest,
        closure: &pdf::PdfRasterClosure,
        code: ErrorCode,
        message: &'static str,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::NoMeaningfulArtifact,
            code,
            message: Box::from(message),
            format: request.format(),
            element_id: Some(closure.element_id.clone()),
            json_pointer: closure.json_pointer.clone().into_boxed_str(),
            details: Some(Box::new(ErrorDetails::PdfRasterFallback {
                recovery: None,
                logical_bounds: Some(closure.logical_bounds),
                pixel_width: Some(closure.pixel_width),
                pixel_height: Some(closure.pixel_height),
                causes: closure.causes.clone(),
            })),
            report: self.take_report(),
        }
    }

    fn account_embeddable_label(
        &mut self,
        outline: &mut ResolvedOutline,
        request: &RenderRequest,
    ) -> Result<(), RenderError> {
        let Some(label) = outline.generated_label.as_ref() else {
            return Ok(());
        };
        let geometry_segments = label.outline_segments();
        let glyphs = label.glyph_count();
        let text_scalars = label.scalar_count;
        let line_break_steps = label.line_break_steps;
        let total_work_units = text_scalars
            .saturating_add(glyphs)
            .saturating_add(line_break_steps)
            .saturating_add(geometry_segments)
            .saturating_mul(32);
        let working_bytes = glyphs
            .saturating_mul(64)
            .saturating_add(line_break_steps.saturating_mul(32))
            .saturating_add(
                u64::try_from(label.lines.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(128),
            )
            .saturating_add(geometry_segments.saturating_mul(64));
        let delta = ResourceDelta {
            geometry_segments,
            working_bytes,
            total_work_units,
            ..ResourceDelta::default()
        };
        let context = ResourceContext::new(
            ResourcePhase::SharedSemanticResolution,
            "embeddable_label_layout_and_glyph_geometry",
            outline.source.pointer(),
        )
        .with_element_id(Some(outline.source.id()))
        .with_local_owner(Some(outline.source.id()));
        if self.meter.would_cross_fatal(delta) {
            self.transact(request, context, delta)?;
            return Err(self.internal_failure(
                request.format(),
                "fatal embeddable label resource preflight committed unexpectedly",
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
                message: "embeddable label was omitted because its glyph outlines exceed the geometry limit",
            })
        } else if outline
            .base_element_work_units
            .saturating_add(total_work_units)
            > request.resource_limits().max_element_work_units()
        {
            Some(ElementLocalCrossing {
                code: DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED,
                field: "max_element_work_units",
                configured: ResourceValue::Integer(
                    request.resource_limits().max_element_work_units(),
                ),
                value_kind: ValueKind::WorkUnits,
                attempted: ResourceValue::Integer(
                    outline
                        .base_element_work_units
                        .saturating_add(total_work_units),
                ),
                pointer_field: None,
                message: "embeddable label was omitted because shaping exceeds its work limit",
            })
        } else {
            None
        };
        if let Some(crossing) = local_crossing {
            let diagnostic = crossing
                .diagnostic(
                    request.format(),
                    Some(outline.source.id()),
                    outline.source.pointer(),
                )
                .map_err(|_| {
                    self.memory_failure(
                        request,
                        "compatibility_report",
                        "embeddable_label_local_recovery",
                    )
                })?;
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::SharedSemanticResolution,
                "embeddable_label_local_resource_recovery",
                diagnostic,
                DiagnosticOwnership {
                    local_owner: Some(outline.source.id()),
                    source_order: Some(outline.source_order),
                    source_position: Some(0),
                },
            )?;
            self.charge_local_omission(
                request,
                Some(outline.source.id()),
                outline.source.pointer(),
                "embeddable_label_local_omission",
            )?;
            outline.generated_label = None;
            outline.generated_label_color = None;
            return Ok(());
        }
        for codepoint in &label.missing_codepoints {
            let diagnostic = official_missing_glyph_diagnostic(
                request.format(),
                outline.source.id(),
                outline.source.pointer(),
                *codepoint,
            )
            .map_err(|_| {
                self.memory_failure(
                    request,
                    "compatibility_report",
                    "embeddable_label_missing_glyph",
                )
            })?;
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::SharedSemanticResolution,
                "embeddable_label_missing_glyph",
                diagnostic,
                DiagnosticOwnership {
                    local_owner: Some(outline.source.id()),
                    source_order: Some(outline.source_order),
                    source_position: Some(u64::MAX),
                },
            )?;
        }
        self.transact(request, context, delta)?;
        outline.base_element_work_units = outline
            .base_element_work_units
            .saturating_add(total_work_units);
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

    fn discard_pending_png_animation_diagnostic(
        &mut self,
        request: &RenderRequest,
        element_id: Option<&str>,
    ) -> Result<(), RenderError> {
        if let Some(element_id) = element_id {
            if let Some(index) = self
                .pending_png_animation_diagnostics
                .iter()
                .position(|pending| pending.local_owner == element_id)
            {
                let pending = self.pending_png_animation_diagnostics.remove(index);
                self.release_pending_png_animation_diagnostic(request, &pending)?;
            }
        }
        Ok(())
    }

    fn flush_pending_png_animation_diagnostics(
        &mut self,
        request: &RenderRequest,
    ) -> Result<(), RenderError> {
        let pending = std::mem::take(&mut self.pending_png_animation_diagnostics);
        for pending in &pending {
            self.release_pending_png_animation_diagnostic(request, pending)?;
        }
        for pending in pending {
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::ProjectionAndFallback,
                "image_animation_flattened",
                pending.diagnostic,
                DiagnosticOwnership {
                    local_owner: Some(&pending.local_owner),
                    source_order: Some(pending.source_order),
                    source_position: Some(u64::MAX),
                },
            )?;
        }
        Ok(())
    }

    fn release_pending_png_animation_diagnostic(
        &mut self,
        request: &RenderRequest,
        pending: &PendingAnimationDiagnostic,
    ) -> Result<(), RenderError> {
        self.release_working(
            request,
            ResourceContext::new(
                ResourcePhase::SharedSemanticResolution,
                "pending_image_animation_flattened",
                &pending.diagnostic.json_pointer,
            )
            .with_element_id(pending.diagnostic.element_id.as_deref())
            .with_local_owner(Some(&pending.local_owner)),
            pending.working_bytes,
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

    #[cfg(test)]
    fn measure_and_emit_output(
        &mut self,
        request: &RenderRequest,
        error_code: ErrorCode,
        codec_emission_work: u64,
        mut encode: impl FnMut() -> Result<Vec<u8>, FormatEncodeError>,
    ) -> Result<(), RenderError> {
        let measured = encode().map_err(|error| self.format_failure(request, error_code, error))?;
        self.emit_premeasured_output(request, error_code, codec_emission_work, measured, encode)
    }

    fn emit_premeasured_output(
        &mut self,
        request: &RenderRequest,
        error_code: ErrorCode,
        codec_emission_work: u64,
        measured: Vec<u8>,
        mut encode: impl FnMut() -> Result<Vec<u8>, FormatEncodeError>,
    ) -> Result<(), RenderError> {
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
        asset_records: bool,
    ) -> Result<(), RenderError> {
        for record in records {
            let is_asset = asset::is_persistent_event(record.event);
            if is_asset != asset_records {
                continue;
            }
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
    mandatory_notices: Option<notices::MandatoryNotices>,
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
    relationship_element_target(
        source_index,
        crate::relationships::RelationshipKind::Frame,
        relationships,
    )
}

fn semantic_identity_hex(identity: &[u8; 32]) -> Result<String, std::collections::TryReserveError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::new();
    encoded.try_reserve_exact(64)?;
    for byte in identity {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn relationship_asset_authorities(
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> Result<Vec<(usize, asset::Authority)>, std::collections::TryReserveError> {
    let mut authorities = Vec::new();
    authorities.try_reserve_exact(relationships.edges.len())?;
    for edge in &relationships.edges {
        if edge.kind != crate::relationships::RelationshipKind::Asset {
            continue;
        }
        let Some(node) = relationships.nodes.get(edge.source) else {
            continue;
        };
        let authority = match edge.target {
            crate::relationships::RelationshipTarget::Asset {
                authority: crate::relationships::AssetAuthority::SceneFile(_),
                ..
            } => asset::Authority::Scene,
            crate::relationships::RelationshipTarget::Asset {
                authority: crate::relationships::AssetAuthority::Supplemental,
                ..
            } => asset::Authority::Supplemental,
            crate::relationships::RelationshipTarget::Asset {
                authority: crate::relationships::AssetAuthority::Missing,
                ..
            } => asset::Authority::Missing,
            _ => continue,
        };
        authorities.push((node.source_index, authority));
    }
    authorities.sort_unstable_by_key(|(source_index, _)| *source_index);
    authorities.dedup_by_key(|(source_index, _)| *source_index);
    Ok(authorities)
}

fn asset_failure_location(
    scene: &SceneDocument,
    supplemental: &crate::SupplementalAssets,
    file_id: &str,
    authority: asset::Authority,
) -> Result<(String, u64, u64, String), std::collections::TryReserveError> {
    match authority {
        asset::Authority::Scene => {
            let source_index = (0..scene.file_record_count())
                .find(|index| {
                    scene
                        .file_record_at(*index)
                        .is_some_and(|record| record.key() == file_id)
                })
                .unwrap_or(usize::MAX);
            let record_pointer = crate::scene::pointer_child("/files", file_id)?;
            let data_position = scene
                .file_record_at(source_index)
                .and_then(|record| record.member_position("dataURL"))
                .and_then(|position| u64::try_from(position).ok())
                .unwrap_or(u64::MAX);
            Ok((
                crate::scene::pointer_child(&record_pointer, "dataURL")?,
                scene.source_owner_order("files", source_index),
                data_position,
                try_owned(file_id)?,
            ))
        }
        asset::Authority::Supplemental => Ok((
            String::new(),
            supplemental_source_order(scene, supplemental, file_id),
            0,
            try_owned(file_id)?,
        )),
        asset::Authority::Missing => Ok((String::new(), u64::MAX, u64::MAX, try_owned(file_id)?)),
    }
}

fn asset_claim_location(
    scene: &SceneDocument,
    supplemental: &crate::SupplementalAssets,
    file_id: &str,
    authority: asset::Authority,
    field: &str,
) -> Result<(String, u64, u64, String), std::collections::TryReserveError> {
    match authority {
        asset::Authority::Scene => {
            let source_index = (0..scene.file_record_count())
                .find(|index| {
                    scene
                        .file_record_at(*index)
                        .is_some_and(|record| record.key() == file_id)
                })
                .unwrap_or(usize::MAX);
            let pointer = crate::scene::pointer_child(
                &crate::scene::pointer_child("/files", file_id)?,
                field,
            )?;
            let source_position = scene
                .file_record_at(source_index)
                .and_then(|record| record.member_position(field))
                .and_then(|position| u64::try_from(position).ok())
                .unwrap_or(u64::MAX);
            Ok((
                pointer,
                scene.source_owner_order("files", source_index),
                source_position,
                try_owned(file_id)?,
            ))
        }
        asset::Authority::Supplemental => Ok((
            String::new(),
            supplemental_source_order(scene, supplemental, file_id),
            0,
            try_owned(file_id)?,
        )),
        asset::Authority::Missing => unreachable!("missing assets have no media claim"),
    }
}

fn supplemental_source_order(
    scene: &SceneDocument,
    supplemental: &crate::SupplementalAssets,
    file_id: &str,
) -> u64 {
    let rank = supplemental
        .iter()
        .position(|(candidate, _)| candidate == file_id)
        .unwrap_or(usize::MAX);
    scene.source_owner_order("files", scene.file_record_count().saturating_add(rank))
}

fn relationship_element_target(
    source_index: usize,
    kind: crate::relationships::RelationshipKind,
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> Option<usize> {
    let node_index = relationships
        .nodes
        .iter()
        .position(|node| node.source_index == source_index)?;
    relationships.edges.iter().find_map(|edge| {
        if edge.source != node_index || edge.kind != kind {
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

#[derive(Clone, Copy)]
struct BoundTextPlacement {
    source_index: usize,
    x: f64,
    y: f64,
    angle: f64,
}

#[derive(Clone, Copy)]
struct BoundTextMaskSource {
    container_source_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn resolve_bound_text_placements(
    outlines: &[(NormalizedElement, Option<GeneratedGeometry>)],
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> Result<Vec<BoundTextPlacement>, ResolveSceneError> {
    let mut placements = Vec::new();
    placements.try_reserve(outlines.len())?;
    for (text, _) in outlines {
        let NormalizedElementKind::Text(text_source) = &text.kind else {
            continue;
        };
        let Some(container_index) = relationship_element_target(
            text.source.source_index(),
            crate::relationships::RelationshipKind::Binding,
            relationships,
        ) else {
            continue;
        };
        let Some((container, geometry)) = outlines
            .iter()
            .find(|(candidate, _)| candidate.source.source_index() == container_index)
        else {
            continue;
        };
        let (x, y, angle) = match &container.kind {
            NormalizedElementKind::Rectangle
            | NormalizedElementKind::Diamond
            | NormalizedElementKind::Ellipse => {
                let [box_x, box_y, box_width, box_height] =
                    bound_text_box(container).ok_or(ResolveSceneError::UnboundedGeometry)?;
                let x = match text_source.text_align.as_str() {
                    "center" => box_x + (box_width - text.width) / 2.0,
                    "right" => box_x + box_width - text.width,
                    _ => box_x,
                };
                let y = match text_source.vertical_align.as_str() {
                    "middle" => box_y + (box_height - text.height) / 2.0,
                    "bottom" => box_y + box_height - text.height,
                    _ => box_y,
                };
                (x, y, container.angle)
            }
            NormalizedElementKind::Linear(linear)
                if linear.kind == crate::normalization::NormalizedLinearKind::Arrow =>
            {
                let Some(midpoint) = linear_midpoint(container, linear, geometry.as_ref()) else {
                    continue;
                };
                let (min_x, min_y, max_x, max_y) = geometry.as_ref().map_or_else(
                    || source_layout_bounds(container),
                    |geometry| geometry.bounds(container),
                );
                let center_x = (min_x + max_x) / 2.0;
                let center_y = (min_y + max_y) / 2.0;
                let delta_x = midpoint[0] - center_x;
                let delta_y = midpoint[1] - center_y;
                let sin = container.angle.sin();
                let cos = container.angle.cos();
                let global_x = container.x + center_x + delta_x * cos - delta_y * sin;
                let global_y = container.y + center_y + delta_x * sin + delta_y * cos;
                (
                    global_x - text.width / 2.0,
                    global_y - text.height / 2.0,
                    text.angle,
                )
            }
            _ => continue,
        };
        placements.push(BoundTextPlacement {
            source_index: text.source.source_index(),
            x,
            y,
            angle,
        });
    }
    Ok(placements)
}

fn bound_text_box(container: &NormalizedElement) -> Option<[f64; 4]> {
    const PADDING: f64 = 5.0;
    let (offset_x, offset_y, width, height) = match container.kind {
        NormalizedElementKind::Rectangle => (
            PADDING,
            PADDING,
            container.width - PADDING * 2.0,
            container.height - PADDING * 2.0,
        ),
        NormalizedElementKind::Ellipse => {
            let inset_x = container.width / 2.0 * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
            let inset_y = container.height / 2.0 * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
            (
                PADDING + inset_x,
                PADDING + inset_y,
                (container.width / 2.0 * std::f64::consts::SQRT_2).round() - PADDING * 2.0,
                (container.height / 2.0 * std::f64::consts::SQRT_2).round() - PADDING * 2.0,
            )
        }
        NormalizedElementKind::Diamond => (
            PADDING + container.width / 4.0,
            PADDING + container.height / 4.0,
            (container.width / 2.0).round() - PADDING * 2.0,
            (container.height / 2.0).round() - PADDING * 2.0,
        ),
        _ => return None,
    };
    let values = [
        container.x + offset_x,
        container.y + offset_y,
        width.max(0.0),
        height.max(0.0),
    ];
    values
        .iter()
        .all(|value| value.is_finite())
        .then_some(values)
}

fn linear_midpoint(
    source: &NormalizedElement,
    linear: &crate::normalization::NormalizedLinear,
    geometry: Option<&GeneratedGeometry>,
) -> Option<[f64; 2]> {
    let points = &linear.points;
    if points.is_empty() {
        return None;
    }
    if points.len() % 2 == 1 {
        return points.get(points.len() / 2).copied();
    }
    let right = points.len() / 2;
    let left_point = points.get(right.checked_sub(1)?)?;
    let right_point = points.get(right)?;
    if points.len() > 2 && source.roundness.is_some() {
        if let Some(GeneratedGeometry::Rough(drawing)) = geometry {
            if let Some(midpoint) = rough_segment_midpoint(&drawing.outline, *right_point) {
                return Some(midpoint);
            }
        }
    }
    Some([
        (left_point[0] + right_point[0]) / 2.0,
        (left_point[1] + right_point[1]) / 2.0,
    ])
}

fn rough_segment_midpoint(path: &rough::Path, target: [f64; 2]) -> Option<[f64; 2]> {
    let mut current = None;
    let mut closest = None;
    let mut closest_distance = f64::INFINITY;
    for operation in &path.operations {
        match *operation {
            rough::PathOperation::Move(point) | rough::PathOperation::Line(point) => {
                current = Some(point);
            }
            rough::PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                let start = current?;
                let distance = (end.x - target[0]).hypot(end.y - target[1]);
                if distance < closest_distance {
                    closest_distance = distance;
                    closest = Some((start, control_1, control_2, end));
                }
                current = Some(end);
            }
        }
    }
    let (start, control_1, control_2, end) = closest?;
    let mut samples = [[0.0; 2]; 21];
    for (index, sample) in samples.iter_mut().enumerate() {
        let t = 1.0 - index as f64 * 0.05;
        *sample = cubic_point(start, control_1, control_2, end, t);
    }
    let mut lengths = [0.0; 21];
    for index in 1..samples.len() {
        lengths[index] = lengths[index - 1]
            + (samples[index][0] - samples[index - 1][0])
                .hypot(samples[index][1] - samples[index - 1][1]);
    }
    let target_length = lengths[20] / 2.0;
    let upper = lengths
        .partition_point(|length| *length < target_length)
        .min(20);
    let lower = upper.saturating_sub(1);
    let span = lengths[upper] - lengths[lower];
    let fraction = if span == 0.0 {
        0.0
    } else {
        (target_length - lengths[lower]) / span
    };
    let sample_position = lower as f64 + fraction;
    let t = 1.0 - sample_position / 20.0;
    Some(cubic_point(start, control_1, control_2, end, t))
}

fn cubic_point(
    start: rough::Point,
    control_1: rough::Point,
    control_2: rough::Point,
    end: rough::Point,
    t: f64,
) -> [f64; 2] {
    let inverse = 1.0 - t;
    let coordinate = |start: f64, control_1: f64, control_2: f64, end: f64| {
        inverse.powi(3) * end
            + 3.0 * t * inverse.powi(2) * control_2
            + 3.0 * t.powi(2) * inverse * control_1
            + t.powi(3) * start
    };
    [
        coordinate(start.x, control_1.x, control_2.x, end.x),
        coordinate(start.y, control_1.y, control_2.y, end.y),
    ]
}

fn has_reciprocal_bound_text(
    container_source_index: usize,
    text_source_index: usize,
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> bool {
    let Some(container_node) = relationships
        .nodes
        .iter()
        .position(|node| node.source_index == container_source_index)
    else {
        return false;
    };
    relationships.edges.iter().any(|edge| {
        if edge.source != container_node
            || edge.kind != crate::relationships::RelationshipKind::Binding
            || !(edge.json_pointer.contains("/boundElements/")
                || edge.json_pointer.contains("/boundElementIds/"))
        {
            return false;
        }
        let crate::relationships::RelationshipTarget::Element(target) = edge.target else {
            return false;
        };
        relationships
            .nodes
            .get(target)
            .is_some_and(|node| node.source_index == text_source_index)
    })
}

fn resolve_bound_text_masks(
    outlines: &[(NormalizedElement, Option<GeneratedGeometry>)],
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> Result<Vec<BoundTextMaskSource>, ResolveSceneError> {
    let mut masks = Vec::new();
    masks.try_reserve(outlines.len())?;
    for (text, _) in outlines {
        if !matches!(text.kind, NormalizedElementKind::Text(_)) {
            continue;
        }
        let Some(container_source_index) = relationship_element_target(
            text.source.source_index(),
            crate::relationships::RelationshipKind::Mask,
            relationships,
        ) else {
            continue;
        };
        let is_arrow = outlines.iter().any(|(candidate, _)| {
            candidate.source.source_index() == container_source_index
                && matches!(
                    candidate.kind,
                    NormalizedElementKind::Linear(crate::normalization::NormalizedLinear {
                        kind: crate::normalization::NormalizedLinearKind::Arrow,
                        ..
                    })
                )
        });
        if is_arrow
            && has_reciprocal_bound_text(
                container_source_index,
                text.source.source_index(),
                relationships,
            )
        {
            masks.push(BoundTextMaskSource {
                container_source_index,
                x: text.x,
                y: text.y,
                width: text.width,
                height: text.height,
            });
        }
    }
    Ok(masks)
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
    InvalidMandatoryNotice,
    InvalidColor,
    UnboundedGeometry,
}

fn format_error_code(format: OutputFormat) -> ErrorCode {
    match format {
        OutputFormat::Svg => ErrorCode::ENCODE_SVG_FAILED,
        OutputFormat::Png => ErrorCode::ENCODE_PNG_FAILED,
        OutputFormat::Pdf => ErrorCode::ENCODE_PDF_FAILED,
    }
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
            mandatory_notices: None,
            _relationships: relationships,
        })
    }

    fn elements(
        mut outlines: Vec<(NormalizedElement, Option<GeneratedGeometry>)>,
        images: ResolvedImageInputs,
        fallbacks: Vec<crate::scene_surface::SurfaceFallback>,
        relationships: crate::relationships::ResolvedRelationshipGraph,
        frame_rendering: EffectiveFrameRendering,
        background_color: &str,
        request: &RenderRequest,
    ) -> Result<Option<Self>, ResolveSceneError> {
        if outlines.is_empty() && fallbacks.is_empty() {
            return Ok(None);
        }

        let text_placements = resolve_bound_text_placements(&outlines, &relationships)?;
        for placement in text_placements {
            let Some((text, _)) = outlines
                .iter_mut()
                .find(|(candidate, _)| candidate.source.source_index() == placement.source_index)
            else {
                continue;
            };
            text.x = placement.x;
            text.y = placement.y;
            text.angle = placement.angle;
        }
        let bound_text_masks = resolve_bound_text_masks(&outlines, &relationships)?;

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
                        link: source.link,
                        width: source.width,
                        height: source.height,
                    }));
                }
                continue;
            }
            if let NormalizedElementKind::Text(text_source) = &source.kind {
                if text_source.text.is_empty() {
                    continue;
                }
                let container_source_index = relationship_element_target(
                    source.source.source_index(),
                    crate::relationships::RelationshipKind::Binding,
                    &relationships,
                );
                if container_source_index.is_some_and(|container| {
                    !has_reciprocal_bound_text(
                        container,
                        source.source.source_index(),
                        &relationships,
                    )
                }) {
                    continue;
                }
                let container_source_order = container_source_index.and_then(|container| {
                    relationships
                        .nodes
                        .iter()
                        .find(|node| node.source_index == container)
                        .map(|node| node.source_order)
                });
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
                    container_source_order,
                    link: source.link,
                    width: source.width,
                    height: source.height,
                }));
                continue;
            }
            if let NormalizedElementKind::Image(image) = &source.kind {
                if let Some(asset) = images.assets.get(&source.source.source_index()) {
                    let source_rect = image.crop.map_or(
                        ResolvedImageSourceRect {
                            x: 0.0,
                            y: 0.0,
                            width: f64::from(asset.width),
                            height: f64::from(asset.height),
                        },
                        |crop| ResolvedImageSourceRect {
                            x: crop.x * f64::from(asset.width) / crop.natural_width,
                            y: crop.y * f64::from(asset.height) / crop.natural_height,
                            width: crop.width * f64::from(asset.width) / crop.natural_width,
                            height: crop.height * f64::from(asset.height) / crop.natural_height,
                        },
                    );
                    elements.push(ResolvedElement::Image(ResolvedImage {
                        source_order: source.source_order,
                        source: source.source,
                        asset: std::sync::Arc::clone(asset),
                        frame_clips,
                        transform: ResolvedTransform {
                            translation_x: source.x - min_x + request.padding(),
                            translation_y: source.y - min_y + request.padding(),
                            angle_radians: source.angle,
                            center_x: source.width / 2.0,
                            center_y: source.height / 2.0,
                        },
                        opacity: source.opacity / 100.0 * frame_opacity,
                        source_rect,
                        scale: image.scale,
                        link: source.link,
                        width: source.width,
                        height: source.height,
                    }));
                } else if let Some((_, status)) = images
                    .fallbacks
                    .iter()
                    .find(|(source_index, _)| *source_index == source.source.source_index())
                {
                    elements.push(ResolvedElement::ImageFallback(ResolvedImageFallback {
                        source_order: source.source_order,
                        source: source.source,
                        status: *status,
                        frame_clips,
                        transform: ResolvedTransform {
                            translation_x: source.x - min_x + request.padding(),
                            translation_y: source.y - min_y + request.padding(),
                            angle_radians: source.angle,
                            center_x: source.width / 2.0,
                            center_y: source.height / 2.0,
                        },
                        opacity: source.opacity / 100.0 * frame_opacity,
                        width: source.width,
                        height: source.height,
                    }));
                }
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
            let source_index = source.source.source_index();
            match geometry {
                GeneratedGeometry::Rough(drawing) => {
                    let (generated_label, generated_label_color, generated_label_offset_y) =
                        if request.format() == OutputFormat::Png
                            && (source.width == 0.0 || source.height == 0.0)
                        {
                            (None, None, 0.0)
                        } else if let NormalizedElementKind::Embeddable { label, .. } = &source.kind
                        {
                            let font_size = embeddable_font_size(source.width, label);
                            let font_index = if let Some(index) = official_fonts
                                .iter()
                                .position(|font| font.matches_family(2))
                            {
                                index
                            } else {
                                official_fonts.try_reserve(1)?;
                                official_fonts.push(text::OfficialFont::load(2, label).map_err(
                                    |error| match error {
                                        text::LabelError::Memory => ResolveSceneError::Memory,
                                        text::LabelError::InvalidFont => {
                                            ResolveSceneError::InvalidFallbackFont
                                        }
                                    },
                                )?);
                                official_fonts.len() - 1
                            };
                            let wrapped_label = official_fonts[font_index]
                                .wrap_embeddable_label(label, font_size, source.width - 20.0)
                                .map_err(|error| match error {
                                    text::LabelError::Memory => ResolveSceneError::Memory,
                                    text::LabelError::InvalidFont => {
                                        ResolveSceneError::InvalidFallbackFont
                                    }
                                })?;
                            let mut laid_out = official_fonts[font_index]
                                .layout(
                                    &wrapped_label,
                                    text::TextLayout {
                                        width: source.width,
                                        font_size,
                                        line_height: 1.15,
                                        text_align: "center",
                                    },
                                )
                                .map_err(|error| match error {
                                    text::LabelError::Memory => ResolveSceneError::Memory,
                                    text::LabelError::InvalidFont => {
                                        ResolveSceneError::InvalidFallbackFont
                                    }
                                })?;
                            laid_out.font_index = font_index;
                            let line_count = laid_out.lines.len() as f64;
                            let offset_y = (source.height - line_count * font_size * 1.15) / 2.0;
                            let label_color =
                                color::parse_css(if source.stroke_color == "transparent" {
                                    "#000000"
                                } else {
                                    &source.stroke_color
                                })
                                .ok_or(ResolveSceneError::InvalidColor)?
                                .for_mode(request.color_mode());
                            (Some(laid_out), Some(label_color), offset_y)
                        } else {
                            (None, None, 0.0)
                        };
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
                        element_opacity: source.opacity / 100.0,
                        background_color,
                        label_mask: bound_text_masks
                            .iter()
                            .find(|mask| mask.container_source_index == source_index)
                            .map(|mask| ResolvedLabelMask {
                                x: mask.x - min_x + request.padding(),
                                y: mask.y - min_y + request.padding(),
                                width: mask.width,
                                height: mask.height,
                            }),
                        width: source.width,
                        height: source.height,
                        generated_label,
                        generated_label_color,
                        generated_label_offset_y,
                        final_pass: matches!(source.kind, NormalizedElementKind::Embeddable { .. }),
                        link: source.link,
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
                        link: source.link,
                        width: source.width,
                        height: source.height,
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
        elements.sort_by_key(ResolvedElement::paint_order);
        let (fallback_font_woff2, fallback_font_sfnt) = if has_label {
            fallback_font
                .map(text::FallbackFont::into_parts)
                .map_or((None, None), |(woff2, sfnt)| (Some(woff2), Some(sfnt)))
        } else {
            (None, None)
        };
        let has_official_text = elements.iter().any(|element| match element {
            ResolvedElement::Text(_) => true,
            ResolvedElement::Outline(outline) => outline.generated_label.is_some(),
            _ => false,
        });
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
        let mandatory_notices = if has_label || has_official_text {
            Some(
                text::mandatory_notices_json(has_label, &resolved_fonts).map_err(|error| {
                    match error {
                        text::LabelError::Memory => ResolveSceneError::Memory,
                        text::LabelError::InvalidFont => ResolveSceneError::InvalidMandatoryNotice,
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
            mandatory_notices,
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
            ResolvedElement::Image(image) => (Some(image.source.id()), image.source.pointer()),
            ResolvedElement::ImageFallback(image) => {
                (Some(image.source.id()), image.source.pointer())
            }
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
    Image(ResolvedImage),
    ImageFallback(ResolvedImageFallback),
    Fallback(ResolvedFallback),
}

impl ResolvedElement {
    fn source_order(&self) -> u64 {
        match self {
            Self::Outline(outline) => outline.source_order,
            Self::Freedraw(freedraw) => freedraw.source_order,
            Self::Frame(frame) => frame.source_order,
            Self::Text(text) => text.source_order,
            Self::Image(image) => image.source_order,
            Self::ImageFallback(image) => image.source_order,
            Self::Fallback(fallback) => fallback.source_order,
        }
    }

    fn paint_order(&self) -> (bool, u64, u8) {
        match self {
            Self::Text(text) => text
                .container_source_order
                .map_or((false, text.source_order, 0), |order| (false, order, 1)),
            Self::Outline(outline) => (outline.final_pass, outline.source_order, 0),
            _ => (false, self.source_order(), 0),
        }
    }
}

struct ResolvedImageAsset {
    identity: [u8; 32],
    source_bytes: std::sync::Arc<Vec<u8>>,
    media_type: &'static str,
    width: u32,
    height: u32,
    rgba: Option<std::sync::Arc<Vec<u8>>>,
    premultiplied_rgba: Option<std::sync::Arc<Vec<u8>>>,
    sanitized_svg: Option<std::sync::Arc<Vec<u8>>>,
    sanitized_svg_tree: Option<std::sync::Arc<usvg::Tree>>,
    svg_effects: Vec<asset::svg::Effect>,
    svg_fallback_pointer: Option<String>,
}

struct ResolvedImageInputs {
    assets: std::collections::BTreeMap<usize, std::sync::Arc<ResolvedImageAsset>>,
    fallbacks: Vec<(usize, crate::normalization::NormalizedImageStatus)>,
}

struct ResolvedImage {
    source_order: u64,
    source: NormalizedElementSource,
    asset: std::sync::Arc<ResolvedImageAsset>,
    frame_clips: Vec<ResolvedClip>,
    transform: ResolvedTransform,
    opacity: f64,
    source_rect: ResolvedImageSourceRect,
    scale: [i8; 2],
    link: Option<String>,
    width: f64,
    height: f64,
}

struct ResolvedImageFallback {
    source_order: u64,
    source: NormalizedElementSource,
    status: crate::normalization::NormalizedImageStatus,
    frame_clips: Vec<ResolvedClip>,
    transform: ResolvedTransform,
    opacity: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy)]
struct ResolvedImageSourceRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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
    container_source_order: Option<u64>,
    link: Option<String>,
    width: f64,
    height: f64,
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
    element_opacity: f64,
    background_color: CanonicalColor,
    label_mask: Option<ResolvedLabelMask>,
    link: Option<String>,
    width: f64,
    height: f64,
    generated_label: Option<text::ResolvedText>,
    generated_label_color: Option<CanonicalColor>,
    generated_label_offset_y: f64,
    final_pass: bool,
}

#[derive(Clone, Copy)]
struct ResolvedLabelMask {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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
    link: Option<String>,
    width: f64,
    height: f64,
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
    link: Option<String>,
    width: f64,
    height: f64,
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

fn embeddable_font_size(width: f64, label: &str) -> f64 {
    let utf16_length = label.encode_utf16().count().max(1) as f64;
    let raw = (width / 2.0).min(width / utf16_length).max(width / 30.0);
    if raw == 0.0 {
        20.0
    } else {
        raw
    }
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

fn image_asset_failure_diagnostic(
    failure: asset::ResolveError,
    format: OutputFormat,
    asset_id: &str,
    element_id: &str,
    json_pointer: &str,
    status: crate::normalization::NormalizedImageStatus,
    limits: &crate::ResourceLimits,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let (code, message, resource) = match failure {
        asset::ResolveError::Malformed => (
            DiagnosticCode::ASSET_MALFORMED_FILE_RECORD,
            "referenced authoritative file payload was malformed",
            None,
        ),
        asset::ResolveError::Unsupported => (
            DiagnosticCode::IMAGE_UNSUPPORTED_FORMAT,
            "referenced image payload was not a supported raster format",
            None,
        ),
        asset::ResolveError::DecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced static PNG payload could not be decoded",
            None,
        ),
        asset::ResolveError::JpegDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced JPEG/JFIF payload could not be decoded",
            None,
        ),
        asset::ResolveError::GifDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced GIF payload could not be decoded",
            None,
        ),
        asset::ResolveError::WebPDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced WebP payload could not be decoded",
            None,
        ),
        asset::ResolveError::BmpDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced BMP payload could not be decoded",
            None,
        ),
        asset::ResolveError::IcoDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced ICO payload could not be decoded",
            None,
        ),
        asset::ResolveError::SvgDecodeFailed => (
            DiagnosticCode::IMAGE_DECODE_FAILED,
            "referenced nested SVG payload could not be parsed and sanitized",
            None,
        ),
        asset::ResolveError::Missing => {
            unreachable!("missing assets are diagnosed by normalization")
        }
        asset::ResolveError::AssetBytes { attempted } => (
            DiagnosticCode::RESOURCE_ASSET_BYTES_EXCEEDED,
            "asset payload occurrence exceeded its byte ceiling",
            Some((
                "max_asset_bytes",
                limits.max_asset_bytes(),
                attempted,
                ValueKind::Bytes,
            )),
        ),
        asset::ResolveError::Dimensions { attempted } => (
            DiagnosticCode::RESOURCE_IMAGE_DIMENSIONS_EXCEEDED,
            "raster dimensions exceeded the local image ceiling",
            Some((
                "max_raster_dimension",
                u64::from(limits.max_raster_dimension()),
                attempted,
                ValueKind::Pixels,
            )),
        ),
        asset::ResolveError::Pixels { attempted } => (
            DiagnosticCode::RESOURCE_IMAGE_PIXELS_EXCEEDED,
            "raster pixels exceeded the local asset ceiling",
            Some((
                "max_decoded_pixels_per_asset",
                limits.max_decoded_pixels_per_asset(),
                attempted,
                ValueKind::Pixels,
            )),
        ),
        asset::ResolveError::AssetWork { attempted } => (
            DiagnosticCode::RESOURCE_ASSET_WORK_EXCEEDED,
            "raster intrinsic work exceeded the local asset ceiling",
            Some((
                "max_asset_work_units",
                limits.max_asset_work_units(),
                attempted,
                ValueKind::WorkUnits,
            )),
        ),
        asset::ResolveError::SvgComplexity { attempted } => (
            DiagnosticCode::RESOURCE_NESTED_SVG_COMPLEXITY_EXCEEDED,
            "nested SVG exceeded the local node ceiling",
            Some((
                "max_nested_svg_nodes",
                u64::from(limits.max_nested_svg_nodes()),
                attempted,
                ValueKind::Count,
            )),
        ),
        asset::ResolveError::SvgDepth { attempted } => (
            DiagnosticCode::RESOURCE_NESTED_ASSET_DEPTH_EXCEEDED,
            "nested SVG resource exceeded the local asset-depth ceiling",
            Some((
                "max_nested_asset_depth",
                u64::from(limits.max_nested_asset_depth()),
                attempted,
                ValueKind::Count,
            )),
        ),
        asset::ResolveError::Memory => unreachable!("memory failure is fatal before diagnostics"),
    };
    let (recovery, recipe_id) = match format {
        OutputFormat::Png => (
            Recovery::Placeholder,
            Some(match status {
                crate::normalization::NormalizedImageStatus::Error => "official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9",
                crate::normalization::NormalizedImageStatus::Pending
                | crate::normalization::NormalizedImageStatus::Saved => "official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d",
            }),
        ),
        OutputFormat::Svg | OutputFormat::Pdf => (Recovery::Omitted, None),
    };
    let mut effects = Vec::new();
    effects.try_reserve_exact(1)?;
    effects.push(DiagnosticEffect::Visual);
    Ok(CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(
            if let Some((field, configured, attempted, value_kind)) = resource {
                ErrorDetails::ResourceLimit {
                    field,
                    configured: ResourceValue::Integer(configured),
                    value_kind,
                    measurement: Measurement::Attempted {
                        value: ResourceValue::Integer(attempted),
                    },
                    recovery: Some(recovery),
                }
            } else {
                ErrorDetails::Asset {
                    asset_id: try_owned(asset_id)?,
                    claimed_media_type: None,
                    validated_media_type: match failure {
                        asset::ResolveError::DecodeFailed => Some(try_owned("image/png")?),
                        asset::ResolveError::JpegDecodeFailed => Some(try_owned("image/jpeg")?),
                        asset::ResolveError::GifDecodeFailed => Some(try_owned("image/gif")?),
                        asset::ResolveError::WebPDecodeFailed => Some(try_owned("image/webp")?),
                        asset::ResolveError::BmpDecodeFailed => Some(try_owned("image/bmp")?),
                        asset::ResolveError::IcoDecodeFailed => Some(try_owned("image/x-icon")?),
                        asset::ResolveError::SvgDecodeFailed => Some(try_owned("image/svg+xml")?),
                        _ => None,
                    },
                    recovery,
                    recipe_id: recipe_id.map(try_owned).transpose()?,
                }
            },
        )),
        order: DiagnosticOrder::default(),
    })
}

fn image_animation_flattened_diagnostic(
    format: OutputFormat,
    asset_id: &str,
    element_id: &str,
    json_pointer: &str,
    media_type: &str,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(1)?;
    effects.push(DiagnosticEffect::Visual);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::IMAGE_ANIMATION_FLATTENED,
        severity: DiagnosticSeverity::Info,
        format,
        effects,
        element_id: Some(try_owned(element_id)?),
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("animated image was flattened to its first animation frame")?,
        details: Some(Box::new(ErrorDetails::Asset {
            asset_id: try_owned(asset_id)?,
            claimed_media_type: None,
            validated_media_type: Some(try_owned(media_type)?),
            recovery: Recovery::Flattened,
            recipe_id: None,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn pdf_raster_fallback_diagnostic(
    closure: &pdf::PdfRasterClosure,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(2)?;
    effects.extend_from_slice(&[DiagnosticEffect::Visual, DiagnosticEffect::Semantic]);
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::PDF_RASTER_FALLBACK,
        severity: DiagnosticSeverity::Warning,
        format: OutputFormat::Pdf,
        effects,
        element_id: Some(try_owned(&closure.element_id)?),
        json_pointer: try_owned(&closure.json_pointer)?,
        message: try_owned("unsupported nested-SVG effect closure was rasterized for PDF")?,
        details: Some(Box::new(ErrorDetails::PdfRasterFallback {
            recovery: Some(Recovery::Rasterized),
            logical_bounds: Some(closure.logical_bounds),
            pixel_width: Some(closure.pixel_width),
            pixel_height: Some(closure.pixel_height),
            causes: closure.causes.clone(),
        })),
        order: DiagnosticOrder::default(),
    })
}

fn unsafe_svg_removal_diagnostic(
    format: OutputFormat,
    asset_id: &str,
    json_pointer: &str,
    removal: &asset::svg::Removal,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut effects = Vec::new();
    effects.try_reserve_exact(if removal.changes_appearance { 2 } else { 1 })?;
    effects.push(DiagnosticEffect::Semantic);
    if removal.changes_appearance {
        effects.push(DiagnosticEffect::Visual);
    }
    let mut asset_chain = Vec::new();
    asset_chain.try_reserve_exact(1)?;
    asset_chain.push(crate::AssetChainEntry {
        asset_id: Some(try_owned(asset_id)?),
        occurrence: 0,
    });
    let mut segments = Vec::new();
    segments.try_reserve_exact(if removal.attribute_ordinal.is_some() {
        2
    } else {
        1
    })?;
    segments.push(crate::NestedPathSegment {
        kind: crate::NestedSegmentKind::Node,
        ordinal: removal.node_ordinal,
        name: None,
    });
    if let Some(ordinal) = removal.attribute_ordinal {
        segments.push(crate::NestedPathSegment {
            kind: crate::NestedSegmentKind::Attribute,
            ordinal,
            name: Some(try_owned(&removal.name)?),
        });
    }
    let mut paths = Vec::new();
    paths.try_reserve_exact(1)?;
    paths.push(crate::NestedPath { segments });
    Ok(CompatibilityDiagnostic {
        code: DiagnosticCode::IMAGE_UNSAFE_SVG_CONTENT_REMOVED,
        severity: DiagnosticSeverity::Warning,
        format,
        effects,
        element_id: None,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned("unsafe nested SVG content was removed before rendering")?,
        details: Some(Box::new(ErrorDetails::NestedSource {
            asset_chain,
            paths,
            recovery: Recovery::Removed,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn nested_image_failure_diagnostic(
    failure: &asset::svg::Failure,
    format: OutputFormat,
    asset_id: &str,
    json_pointer: &str,
    limits: &crate::ResourceLimits,
) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
    let mut diagnostic = image_asset_failure_diagnostic(
        failure.error,
        format,
        asset_id,
        "",
        json_pointer,
        crate::normalization::NormalizedImageStatus::Saved,
        limits,
    )?;
    let mut asset_chain = Vec::new();
    asset_chain.try_reserve_exact(1)?;
    asset_chain.push(crate::AssetChainEntry {
        asset_id: Some(try_owned(asset_id)?),
        occurrence: 0,
    });
    let mut segments = Vec::new();
    segments.try_reserve_exact(failure.path.len().saturating_mul(2))?;
    for (node_ordinal, attribute_ordinal) in &failure.path {
        segments.push(crate::NestedPathSegment {
            kind: crate::NestedSegmentKind::Node,
            ordinal: *node_ordinal,
            name: None,
        });
        segments.push(crate::NestedPathSegment {
            kind: crate::NestedSegmentKind::Attribute,
            ordinal: *attribute_ordinal,
            name: Some(try_owned("href")?),
        });
    }
    let mut paths = Vec::new();
    paths.try_reserve_exact(1)?;
    paths.push(crate::NestedPath { segments });
    diagnostic.element_id = None;
    diagnostic.message = try_owned("nested data image was removed after local recovery")?;
    diagnostic.details = Some(Box::new(ErrorDetails::NestedSource {
        asset_chain,
        paths,
        recovery: Recovery::Removed,
    }));
    Ok(diagnostic)
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
        NormalizedElementKind::Rectangle | NormalizedElementKind::Embeddable { .. } => {
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

fn artifact_retention_delta(output_bytes: u64, codec_emission_work: u64) -> ResourceDelta {
    ResourceDelta {
        working_bytes: 512_u64.saturating_add(logical_aligned_bytes(output_bytes)),
        output_bytes,
        total_work_units: codec_emission_work.saturating_add(output_bytes),
        ..ResourceDelta::default()
    }
}

fn optional_metadata_omitted_diagnostic(
    format: OutputFormat,
    crossing: crate::resource_accounting::OptionalMetadataCrossing,
) -> CompatibilityDiagnostic {
    let measurement = match crossing.measurement {
        Measurement::Attempted { value } => Measurement::Attempted {
            value: ResourceValue::Integer(value),
        },
        Measurement::AtLeast { value } => Measurement::AtLeast {
            value: ResourceValue::Integer(value),
        },
    };
    CompatibilityDiagnostic {
        code: DiagnosticCode::RESOURCE_OPTIONAL_METADATA_OMITTED,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: vec![DiagnosticEffect::Metadata],
        element_id: None,
        json_pointer: String::new(),
        message: "optional artifact metadata was omitted because it exceeds a resource limit"
            .to_owned(),
        details: Some(Box::new(ErrorDetails::ResourceLimit {
            field: crossing.field,
            configured: ResourceValue::Integer(crossing.configured),
            value_kind: crossing.value_kind,
            measurement,
            recovery: Some(Recovery::MetadataOmitted),
        })),
        order: DiagnosticOrder::default(),
    }
}

fn metadata_component_failure_diagnostic(
    format: OutputFormat,
    component: OptionalMetadataComponent,
    operation: MetadataOperation,
) -> CompatibilityDiagnostic {
    let code = component.serialization_code();
    CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: vec![DiagnosticEffect::Metadata],
        element_id: None,
        json_pointer: String::new(),
        message: "optional artifact metadata component could not be retained".to_owned(),
        details: Some(Box::new(ErrorDetails::Metadata {
            component: component.manifest_component(),
            operation,
            recovery: Recovery::MetadataOmitted,
        })),
        order: DiagnosticOrder::default(),
    }
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
        assert_eq!(approval["approved_at"], "2026-07-23");
        assert_eq!(
            approval["approval_url"],
            "https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-5060283812"
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
                "e314b25a1c82ee51f290fa67e00d53143ca41d8478d57824b494d9306832753a",
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
