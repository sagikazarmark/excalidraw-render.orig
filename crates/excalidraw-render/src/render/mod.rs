mod asset;
mod bound_text;
mod diagnostics;
#[cfg(test)]
pub(crate) use diagnostics::sort_diagnostics;
mod encode;
mod font_demand;
mod font_subset;
#[cfg(test)]
mod font_subset_fuzz;
// The SVG font subset viewer qualification manifest pins this module's digest
// (`tests/conformance/svg-font-subset-viewer-qualification/revision-1.json`),
// so it only changes with a re-qualification. It borrows the face range tables
// the way the former `Vec` return required, which is now a needless borrow of a
// `&'static` slice.
#[cfg(test)]
#[allow(clippy::needless_borrow)]
mod font_subset_qualification;
#[cfg(all(test, target_arch = "wasm32"))]
pub(crate) use font_subset_fuzz::{
    assert_font_subset_fuzz_regression_corpus, assert_font_subset_fuzz_smoke,
};
#[cfg(all(test, target_arch = "wasm32"))]
pub(crate) use font_subset_qualification::assert_complete_font_subset_qualification_corpus;
#[cfg(test)]
mod frame_batch_property;
mod frame_target;
mod freedraw;
mod geometry;
mod image;
mod metadata;
mod notices;
mod pdf;
#[cfg(test)]
mod plan_tests;
mod png;
mod prepared;
pub(crate) use prepared::{FramePreparationError, PreparedFrameRender};
#[cfg(test)]
mod resolve_tests;
#[cfg(test)]
mod resource_trace_qualification;
#[cfg(all(test, target_arch = "wasm32"))]
pub(crate) use resource_trace_qualification::assert_font_subset_resource_trace_matrix;
mod resolution;
mod resolve;
mod rough;
mod svg;
mod svg_fonts;
#[cfg(test)]
mod tests;
mod text;

use crate::{
    catalog::RevisionCatalog,
    color::{self, CanonicalColor},
    normalization::{NormalizedElement, NormalizedElementKind, NormalizedElementSource},
    outcome::{DiagnosticAttribution, DiagnosticOrder},
    resource_accounting::{
        ResourceAccount, ResourceContext, ResourceCrossing, ResourceDelta, ResourceMeter,
        ResourcePhase, ResourceRecordKind,
    },
    scene::OriginalJsonVisitError,
    Artifact, ColorMode, CompatibilityDiagnostic, CompatibilityReport, DiagnosticCode,
    DiagnosticEffect, DiagnosticSeverity, ErrorCode, ErrorDetails, FiniteValue, Measurement,
    MetadataComponent, MetadataOperation, OutputFormat, Recovery, ReferenceProfileIdentity,
    RenderError, RenderErrorCategory, RenderOutcome, RenderRequest, ResourceValue, SceneDocument,
    Sizing, ValueKind,
};
use std::sync::{Arc, Mutex};

use self::{
    bound_text::*, diagnostics::*, font_demand::*, frame_target::*, geometry::*, pdf::PdfPlan,
    png::PngPlan, resolve::*, svg::SvgPlan,
};

#[derive(Debug)]
pub(super) enum FormatEncodeError {
    Encoding(String),
    SvgFontSubset {
        stage: crate::SvgFontSubsetStage,
        resolved_family: &'static str,
        asset_identity: &'static str,
    },
    Metadata(MetadataFailure),
    MemoryUnavailable {
        subject_kind: &'static str,
        subject: &'static str,
    },
}

#[derive(Debug)]
pub(super) struct MetadataFailure {
    code: DiagnosticCode,
    component: MetadataComponent,
    operation: MetadataOperation,
}

pub(super) const fn metadata_embedding_error(
    code: DiagnosticCode,
    component: MetadataComponent,
) -> FormatEncodeError {
    FormatEncodeError::Metadata(MetadataFailure {
        code,
        component,
        operation: MetadataOperation::Embedding,
    })
}

pub(crate) fn render(
    catalog: &'static RevisionCatalog,
    scene: &SceneDocument,
    request: &RenderRequest,
) -> Result<RenderOutcome, RenderError> {
    render_with_svg_font_payload(catalog, scene, request, svg_fonts::Payload::Subset)
}

fn render_with_svg_font_payload(
    catalog: &'static RevisionCatalog,
    scene: &SceneDocument,
    request: &RenderRequest,
    svg_font_payload: svg_fonts::Payload,
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
    session.render(scene, request, None, svg_font_payload)
}

pub(crate) fn render_frame(
    catalog: &'static RevisionCatalog,
    frame: crate::FrameRef<'_>,
    request: &crate::FrameRenderRequest,
) -> Result<RenderOutcome, RenderError> {
    render_frame_scoped(catalog, frame, request, None).0
}

pub(crate) fn render_frame_scoped(
    catalog: &'static RevisionCatalog,
    frame: crate::FrameRef<'_>,
    request: &crate::FrameRenderRequest,
    prepared: Option<&PreparedFrameRender>,
) -> (
    Result<RenderOutcome, RenderError>,
    Vec<CompatibilityDiagnostic>,
    Vec<CompatibilityDiagnostic>,
) {
    let profile = catalog.frame_profile();
    let target = FrameTarget {
        source_index: frame.source_index,
        ordinal: frame.ordinal(),
        kind: frame.kind(),
    };
    let request = request.as_render_request();
    let mut session = RenderSession::new_target(
        profile,
        catalog.scene_surface(),
        frame.scene,
        request,
        RenderTarget::Frame(target),
    );
    if let Some(prepared) = prepared {
        session.font_subset_session = Arc::clone(&prepared.font_subset_session);
    }
    let result = session.render(frame.scene, request, prepared, svg_fonts::Payload::Subset);
    let report = match &result {
        Ok(outcome) => outcome.report(),
        Err(error) => error.report(),
    };
    let global_diagnostics = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.order.attribution == DiagnosticAttribution::SceneGlobal)
        .cloned()
        .collect();
    let target_diagnostics = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.order.attribution == DiagnosticAttribution::Target)
        .cloned()
        .collect();
    (result, global_diagnostics, target_diagnostics)
}

#[derive(Clone, Copy)]
enum RenderTarget {
    Scene,
    Frame(FrameTarget),
}

type SharedFontSubsetSession = Arc<Mutex<svg_fonts::Session>>;
/// Sanitized-SVG viewer trees, keyed by asset identity. The sanitized bytes
/// are contract state and live in the resolved scene; the parsed tree is a
/// deterministic derivation of those bytes that only the raster-capable
/// backends consume, so it lives in the request-scoped session's caches, not
/// in the shared semantic graph (spec section 9).
type SvgTreeCache = std::collections::BTreeMap<[u8; 32], std::sync::Arc<usvg::Tree>>;

fn manifest_target<'a>(
    target: RenderTarget,
    scene: &'a SceneDocument,
) -> metadata::ManifestTarget<'a> {
    match target {
        RenderTarget::Scene => metadata::ManifestTarget::Scene,
        RenderTarget::Frame(target) => {
            let name = scene
                .element_at(target.source_index)
                .and_then(|source| match source.name {
                    crate::scene::SourceField::Value(name) => name,
                    crate::scene::SourceField::Missing | crate::scene::SourceField::Invalid => None,
                });
            metadata::ManifestTarget::Frame {
                ordinal: target.ordinal,
                kind: target.kind,
                name,
            }
        }
    }
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
    let result = session.render(scene, request, None, svg_fonts::Payload::Subset);
    (result, session.meter.trace_bytes())
}

/// The format-neutral semantic graph (spec §9) one render of `scene` under
/// `request` would encode: admission, resolution and fallback accounting run
/// exactly as `RenderSession::render` runs them, and the session stops before
/// planning and encoding. Tests assert pre-encode behavior here once instead
/// of once per format.
#[cfg(test)]
fn resolve_for_test(
    scene: &SceneDocument,
    request: &RenderRequest,
) -> Result<ResolvedScene, RenderError> {
    resolve_with_svg_trees_for_test(scene, request).map(|(resolved, _)| resolved)
}

/// [`resolve_for_test`] together with the nested-SVG trees resolution parsed,
/// which `PngPlan::new` and `PdfPlan::new` take beside the resolved scene.
#[cfg(test)]
fn resolve_with_svg_trees_for_test(
    scene: &SceneDocument,
    request: &RenderRequest,
) -> Result<(ResolvedScene, SvgTreeCache), RenderError> {
    let mut session = RenderSession::new(
        crate::catalog::REVISION_1.profile(),
        crate::catalog::REVISION_1.scene_surface(),
        scene,
        request,
    );
    session.admit_session(scene, request, None)?;
    let mut semantic_working = Vec::new();
    let mut resolved = session.resolve_scene(scene, request, None, &mut semantic_working)?;
    session.account_fallback_semantics(&mut resolved, request, &mut semantic_working)?;
    Ok((resolved, session.svg_trees))
}

#[cfg(test)]
fn render_font_subset_qualification_trace(
    record: &'static excalidraw_render_assets_base::AssetRecord,
    scalars: &[u32],
) -> (Result<(), RenderError>, Vec<u8>) {
    let family = match record.family {
        "Cascadia" => "Cascadia Code",
        "ComicShanns" => "Comic Shanns",
        "Liberation" => "Liberation Sans",
        "Lilita" => "Lilita One",
        "Excalifont" | "Nunito" | "Virgil" | "Xiaolai" => record.family,
        _ => panic!("unknown qualification source family"),
    };
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#,
    )
    .expect("qualification trace scene is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("qualification SVG request is valid");
    let profile = crate::Renderer::profile_identity(crate::CompatibilityRevision::REVISION_1)
        .expect("revision 1 exists");
    let mut session = RenderSession::new_with_trace(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );
    let result = (|| {
        let selection = svg_fonts::account_demand(
            &mut RenderResourceAccount::new(&mut session, &request),
            std::iter::once((family, scalars.len())),
        )?;
        let demand = svg_fonts::Demand {
            families: vec![
                svg_fonts::FamilyDemand::from_scalars(family, scalars).map_err(|_| {
                    session.memory_failure(&request, "svg_font_demand", "family_scalar_set")
                })?,
            ],
        };
        let selection = svg_fonts::account_selection(
            &mut RenderResourceAccount::new(&mut session, &request),
            &demand,
            selection,
        )?;
        let envelope = svg_fonts::account_dependency_envelope(
            &mut RenderResourceAccount::new(&mut session, &request),
            &demand,
        )?;
        let mut subset_session = svg_fonts::Session::new();
        let bundle = svg_fonts::build(demand, &[], svg_fonts::Payload::Subset, &mut subset_session)
            .map_err(|error| {
                session.format_failure(
                    &request,
                    ErrorCode::ENCODE_SVG_FAILED,
                    FormatEncodeError::SvgFontSubset {
                        stage: error.stage,
                        resolved_family: error.family,
                        asset_identity: error.asset_identity,
                    },
                )
            })?;
        bundle
            .account_resource_transcript(&mut RenderResourceAccount::new(&mut session, &request))?;
        release_svg_font_encoding_records(&mut session, &request, None, envelope)?;
        selection.release(&mut RenderResourceAccount::new(&mut session, &request))
    })();
    (result, session.meter.trace_bytes())
}

#[cfg(test)]
fn font_subset_qualification_error(
    stage: crate::SvgFontSubsetStage,
    resolved_family: &'static str,
    asset_identity: &'static str,
) -> RenderError {
    let scene = SceneDocument::parse(
        br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#,
    )
    .expect("qualification failure scene is valid");
    let request = RenderRequest::builder(OutputFormat::Svg)
        .build()
        .expect("qualification failure request is valid");
    let mut session = RenderSession::new(
        crate::catalog::REVISION_1.profile(),
        crate::catalog::REVISION_1.scene_surface(),
        &scene,
        &request,
    );
    session.format_failure(
        &request,
        ErrorCode::ENCODE_SVG_FAILED,
        FormatEncodeError::SvgFontSubset {
            stage,
            resolved_family,
            asset_identity,
        },
    )
}

#[cfg(test)]
fn new_font_subset_cache() -> SharedFontSubsetSession {
    Arc::new(Mutex::new(svg_fonts::Session::new()))
}

#[cfg(test)]
fn render_with_trace_and_font_subset_cache(
    profile: &'static ReferenceProfileIdentity,
    scene: &SceneDocument,
    request: &RenderRequest,
    cache: &SharedFontSubsetSession,
) -> (Result<RenderOutcome, RenderError>, Vec<u8>) {
    let mut session = RenderSession::new_with_trace(
        profile,
        crate::catalog::REVISION_1.scene_surface(),
        scene,
        request,
    );
    session.font_subset_session = Arc::clone(cache);
    let result = session.render(scene, request, None, svg_fonts::Payload::Subset);
    (result, session.meter.trace_bytes())
}

#[cfg(test)]
fn font_subset_cache_stats(cache: &SharedFontSubsetSession) -> (usize, usize) {
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .stats()
}

#[cfg(test)]
fn render_frame_with_trace(
    catalog: &'static RevisionCatalog,
    frame: crate::FrameRef<'_>,
    request: &crate::FrameRenderRequest,
    prepared: Option<&PreparedFrameRender>,
) -> (Result<RenderOutcome, RenderError>, Vec<u8>) {
    let profile = catalog.frame_profile();
    let request = request.as_render_request();
    let mut session =
        RenderSession::new_with_trace(profile, catalog.scene_surface(), frame.scene, request);
    session.target = RenderTarget::Frame(FrameTarget {
        source_index: frame.source_index,
        ordinal: frame.ordinal(),
        kind: frame.kind(),
    });
    if let Some(prepared) = prepared {
        session.font_subset_session = Arc::clone(&prepared.font_subset_session);
    }
    let result = session.render(frame.scene, request, prepared, svg_fonts::Payload::Subset);
    (result, session.meter.trace_bytes())
}

#[cfg(test)]
fn prepared_font_subset_cache_stats(prepared: &PreparedFrameRender) -> (usize, usize) {
    prepared
        .font_subset_session
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .stats()
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
    pending_frame_normalization_diagnostics: Vec<PendingNormalizationDiagnostic>,
    target: RenderTarget,
    diagnostic_attribution: DiagnosticAttribution,
    font_subset_session: SharedFontSubsetSession,
    svg_trees: SvgTreeCache,
}

struct RenderResourceAccount<'a> {
    session: &'a mut RenderSession,
    request: &'a RenderRequest,
}

impl<'a> RenderResourceAccount<'a> {
    fn new(session: &'a mut RenderSession, request: &'a RenderRequest) -> Self {
        Self { session, request }
    }
}

impl ResourceAccount for RenderResourceAccount<'_> {
    type Error = RenderError;

    fn transact_resource(
        &mut self,
        context: ResourceContext<'_>,
        delta: ResourceDelta,
    ) -> Result<(), Self::Error> {
        self.session.transact(self.request, context, delta)
    }

    fn release_resource(
        &mut self,
        context: ResourceContext<'_>,
        bytes: u64,
    ) -> Result<(), Self::Error> {
        self.session.release_working(self.request, context, bytes)
    }

    fn internal_failure(&mut self, message: &'static str) -> Self::Error {
        self.session
            .internal_failure(self.request.format(), message, "")
    }
}

struct PendingAnimationDiagnostic {
    diagnostic: CompatibilityDiagnostic,
    local_owner: String,
    source_order: u64,
    working_bytes: u64,
}

struct PendingNormalizationDiagnostic {
    diagnostic: CompatibilityDiagnostic,
    local_owner: Option<String>,
    source_order: u64,
    source_position: u64,
}

#[derive(Clone, Copy)]
enum OptionalMetadataComponent {
    Provenance,
    DiagnosticSummary,
    EmbeddedScene,
}

trait ArtifactMeasurement {
    const DEFERS_MANDATORY_OUTPUT_ADMISSION: bool;

    fn output_len(&self) -> usize;
}

impl ArtifactMeasurement for usize {
    const DEFERS_MANDATORY_OUTPUT_ADMISSION: bool = true;

    fn output_len(&self) -> usize {
        *self
    }
}

impl ArtifactMeasurement for Vec<u8> {
    const DEFERS_MANDATORY_OUTPUT_ADMISSION: bool = false;

    fn output_len(&self) -> usize {
        self.len()
    }
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

    const fn measurement_event(self) -> &'static str {
        match self {
            Self::Provenance => "provenance_metadata_measurement",
            Self::DiagnosticSummary => "diagnostic_summary_metadata_measurement",
            Self::EmbeddedScene => "embedded_scene_metadata_measurement",
        }
    }

    const fn retention_event(self) -> &'static str {
        match self {
            Self::Provenance => "provenance_metadata_retention",
            Self::DiagnosticSummary => "diagnostic_summary_metadata_retention",
            Self::EmbeddedScene => "embedded_scene_metadata_retention",
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
        Self::new_target(profile, scene_surface, scene, request, RenderTarget::Scene)
    }

    fn new_target(
        profile: &'static ReferenceProfileIdentity,
        scene_surface: &'static crate::scene_surface::SceneSurfacePolicy,
        scene: &SceneDocument,
        request: &RenderRequest,
        target: RenderTarget,
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
            pending_frame_normalization_diagnostics: Vec::new(),
            target,
            diagnostic_attribution: DiagnosticAttribution::SceneGlobal,
            font_subset_session: Arc::new(Mutex::new(svg_fonts::Session::new())),
            svg_trees: SvgTreeCache::new(),
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
            pending_frame_normalization_diagnostics: Vec::new(),
            target: RenderTarget::Scene,
            diagnostic_attribution: DiagnosticAttribution::SceneGlobal,
            font_subset_session: Arc::new(Mutex::new(svg_fonts::Session::new())),
            svg_trees: SvgTreeCache::new(),
        }
    }

    fn render(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        prepared: Option<&PreparedFrameRender>,
        svg_font_payload: svg_fonts::Payload,
    ) -> Result<RenderOutcome, RenderError> {
        self.admit_session(scene, request, prepared)?;
        let mut semantic_working = Vec::new();
        let mut resolved = self.resolve_scene(scene, request, prepared, &mut semantic_working)?;
        self.account_fallback_semantics(&mut resolved, request, &mut semantic_working)?;
        let bytes = self.encode(
            resolved,
            scene,
            request,
            &semantic_working,
            svg_font_payload,
        )?;

        Ok(RenderOutcome {
            artifact: Artifact {
                format: request.format(),
                bytes,
            },
            report: self.take_report(),
            profile: self.profile,
        })
    }

    /// Charges the fixed per-request session cost and replays or performs the
    /// original-JSON traversal that seeds semantic accounting.
    fn admit_session(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        prepared: Option<&PreparedFrameRender>,
    ) -> Result<(), RenderError> {
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
        if let Some(prepared) = prepared {
            for event in &prepared.original_json_events {
                self.observe_original_json_event(
                    request,
                    event.kind,
                    &event.json_pointer,
                    event.is_element_occurrence,
                )?;
            }
        } else {
            let traversal = scene.visit_original_json_entries(|entry| {
                self.observe_original_json_event(
                    request,
                    entry.kind,
                    entry.json_pointer,
                    entry.is_element_occurrence,
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
        }
        Ok(())
    }

    fn encode(
        &mut self,
        resolved: ResolvedScene,
        scene: &SceneDocument,
        request: &RenderRequest,
        semantic_working: &[SemanticWorkingRecord],
        svg_font_payload: svg_fonts::Payload,
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
                self.encode_svg(resolved, scene, request, semantic_working, svg_font_payload)?;
            }
            OutputFormat::Png => self.encode_png(resolved, scene, request, semantic_working)?,
            OutputFormat::Pdf => self.encode_pdf(resolved, scene, request, semantic_working)?,
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
    fn select_artifact_metadata<M: ArtifactMeasurement>(
        &mut self,
        scene: &SceneDocument,
        request: &RenderRequest,
        mandatory_notices: Option<&notices::MandatoryNotices>,
        error_code: ErrorCode,
        codec_emission_work: u64,
        mut measure: impl FnMut(&metadata::ArtifactMetadata) -> Result<M, FormatEncodeError>,
    ) -> Result<(metadata::ArtifactMetadata, M), RenderError> {
        let snapshot = self.report.clone();
        let manifest_target = manifest_target(self.target, scene);
        let requested = metadata::MetadataSelection::requested(request);
        let mut accepted = metadata::MetadataSelection::default();
        let baseline_estimate = metadata::ArtifactMetadata::estimate(
            scene,
            self.profile,
            &snapshot,
            mandatory_notices,
            accepted,
            manifest_target,
        );
        let baseline_context = ResourceContext::new(
            ResourcePhase::Metadata,
            "mandatory_notice_metadata_generation",
            "",
        );
        if mandatory_notices.is_some() {
            let baseline_delta = ResourceDelta {
                working_bytes: baseline_estimate.scratch_bytes,
                total_work_units: baseline_estimate.generation_work,
                ..ResourceDelta::default()
            };
            if self
                .meter
                .optional_metadata_crossing(baseline_context, baseline_delta)
                .is_some()
            {
                return Err(self.format_failure(
                    request,
                    error_code,
                    FormatEncodeError::Encoding(
                        "mandatory notice metadata cannot be generated".to_owned(),
                    ),
                ));
            }
            self.transact(request, baseline_context, baseline_delta)?;
        }
        let baseline = metadata::ArtifactMetadata::build(
            scene,
            self.profile,
            &snapshot,
            mandatory_notices,
            accepted,
            manifest_target,
        )
        .map_err(|error| self.format_failure(request, error_code, error))?;
        let baseline_artifact =
            measure(&baseline).map_err(|error| self.format_failure(request, error_code, error))?;
        let baseline_bytes = u64::try_from(baseline_artifact.output_len()).unwrap_or(u64::MAX);
        if self
            .meter
            .optional_metadata_crossing(
                ResourceContext::new(
                    ResourcePhase::Metadata,
                    "mandatory_notice_metadata_retention",
                    "",
                ),
                artifact_retention_delta(baseline_bytes, codec_emission_work),
            )
            .is_some()
        {
            if mandatory_notices.is_some() && !M::DEFERS_MANDATORY_OUTPUT_ADMISSION {
                return Err(self.format_failure(
                    request,
                    error_code,
                    FormatEncodeError::Encoding(
                        "mandatory notice cannot be retained in the artifact".to_owned(),
                    ),
                ));
            }
            if mandatory_notices.is_none() {
                return Ok((baseline, baseline_artifact));
            }
        }
        if mandatory_notices.is_some() {
            self.release_working(request, baseline_context, baseline_estimate.scratch_bytes)?;
        }
        let mut accepted_metadata = baseline;
        let mut accepted_artifact = baseline_artifact;
        let mut accepted_estimate = if mandatory_notices.is_some() {
            baseline_estimate
        } else {
            metadata::MetadataEstimate::default()
        };

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
            let candidate_estimate = metadata::ArtifactMetadata::estimate(
                scene,
                self.profile,
                &snapshot,
                mandatory_notices,
                candidate_selection,
                manifest_target,
            );
            let compression_workspace = u64::from(matches!(
                component,
                OptionalMetadataComponent::EmbeddedScene
            )) * 1_048_576;
            let scratch_bytes = candidate_estimate
                .scratch_bytes
                .saturating_sub(accepted_estimate.scratch_bytes)
                .saturating_add(compression_workspace);
            let generation_work = candidate_estimate
                .generation_work
                .saturating_sub(accepted_estimate.generation_work);
            let generation_delta = ResourceDelta {
                working_bytes: scratch_bytes,
                total_work_units: generation_work,
                ..ResourceDelta::default()
            };
            if let Some(crossing) = self.meter.optional_metadata_crossing(
                ResourceContext::new(ResourcePhase::Metadata, component.event(), ""),
                generation_delta,
            ) {
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
            let candidate = match metadata::ArtifactMetadata::build(
                scene,
                self.profile,
                &snapshot,
                mandatory_notices,
                candidate_selection,
                manifest_target,
            ) {
                Ok(candidate) => candidate,
                Err(error @ FormatEncodeError::MemoryUnavailable { .. }) => {
                    return Err(self.format_failure(request, error_code, error));
                }
                Err(error @ FormatEncodeError::SvgFontSubset { .. }) => {
                    return Err(self.format_failure(request, error_code, error));
                }
                Err(FormatEncodeError::Metadata(failure)) => {
                    self.release_working(request, generation_context, scratch_bytes)?;
                    if failure.component == MetadataComponent::Manifest {
                        if mandatory_notices.is_some() {
                            return Err(self.format_failure(
                                request,
                                error_code,
                                FormatEncodeError::Encoding(
                                    "mandatory notice manifest could not be serialized".to_owned(),
                                ),
                            ));
                        }
                        self.retain_diagnostic(
                            request,
                            ResourcePhase::Metadata,
                            "artifact_manifest_metadata_omitted",
                            metadata_failure_diagnostic(request.format(), failure),
                        )?;
                        let empty = metadata::ArtifactMetadata::empty();
                        let artifact = measure(&empty)
                            .map_err(|error| self.format_failure(request, error_code, error))?;
                        return Ok((empty, artifact));
                    }
                    self.retain_diagnostic(
                        request,
                        ResourcePhase::Metadata,
                        component.event(),
                        metadata_failure_diagnostic(request.format(), failure),
                    )?;
                    continue;
                }
                Err(error @ FormatEncodeError::Encoding(_)) => {
                    self.release_working(request, generation_context, scratch_bytes)?;
                    return Err(self.format_failure(request, error_code, error));
                }
            };
            let measurement_delta = ResourceDelta {
                total_work_units: generation_work,
                ..ResourceDelta::default()
            };
            if let Some(crossing) = self.meter.optional_metadata_crossing(
                ResourceContext::new(ResourcePhase::Metadata, component.measurement_event(), ""),
                measurement_delta,
            ) {
                self.release_working(request, generation_context, scratch_bytes)?;
                self.retain_diagnostic(
                    request,
                    ResourcePhase::Metadata,
                    component.event(),
                    optional_metadata_omitted_diagnostic(request.format(), crossing),
                )?;
                continue;
            }
            self.transact(
                request,
                ResourceContext::new(ResourcePhase::Metadata, component.measurement_event(), ""),
                measurement_delta,
            )?;
            let measured = measure(&candidate);
            let candidate_artifact = match measured {
                Ok(candidate_artifact) => candidate_artifact,
                Err(error @ FormatEncodeError::MemoryUnavailable { .. }) => {
                    return Err(self.format_failure(request, error_code, error));
                }
                Err(error @ FormatEncodeError::SvgFontSubset { .. }) => {
                    return Err(self.format_failure(request, error_code, error));
                }
                Err(FormatEncodeError::Metadata(failure)) => {
                    self.release_working(request, generation_context, scratch_bytes)?;
                    if failure.component == MetadataComponent::Manifest {
                        if mandatory_notices.is_some() {
                            return Err(self.format_failure(
                                request,
                                error_code,
                                FormatEncodeError::Encoding(
                                    "mandatory notice manifest could not be embedded".to_owned(),
                                ),
                            ));
                        }
                        self.retain_diagnostic(
                            request,
                            ResourcePhase::Metadata,
                            "artifact_manifest_metadata_omitted",
                            metadata_failure_diagnostic(request.format(), failure),
                        )?;
                        let empty = metadata::ArtifactMetadata::empty();
                        let artifact = measure(&empty)
                            .map_err(|error| self.format_failure(request, error_code, error))?;
                        return Ok((empty, artifact));
                    }
                    self.retain_diagnostic(
                        request,
                        ResourcePhase::Metadata,
                        component.event(),
                        metadata_failure_diagnostic(request.format(), failure),
                    )?;
                    continue;
                }
                Err(error @ FormatEncodeError::Encoding(_)) => {
                    self.release_working(request, generation_context, scratch_bytes)?;
                    return Err(self.format_failure(request, error_code, error));
                }
            };
            let marginal_bytes = candidate_artifact
                .output_len()
                .checked_sub(accepted_artifact.output_len())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or_else(|| {
                    self.internal_failure(
                        request.format(),
                        "optional metadata reduced the encoded artifact length",
                        "",
                    )
                })?;
            let output_bytes = u64::try_from(accepted_artifact.output_len())
                .unwrap_or(u64::MAX)
                .saturating_add(marginal_bytes);
            if let Some(crossing) = self.meter.optional_metadata_crossing(
                ResourceContext::new(ResourcePhase::Metadata, component.retention_event(), ""),
                artifact_retention_delta(output_bytes, codec_emission_work),
            ) {
                self.release_working(request, generation_context, scratch_bytes)?;
                self.retain_diagnostic(
                    request,
                    ResourcePhase::Metadata,
                    component.event(),
                    optional_metadata_omitted_diagnostic(request.format(), crossing),
                )?;
            } else {
                self.release_working(request, generation_context, scratch_bytes)?;
                accepted = candidate_selection;
                accepted_metadata = candidate;
                accepted_artifact = candidate_artifact;
                accepted_estimate = candidate_estimate;
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
            if let ResolvedElement::Image(image) = element {
                // A rounded image's corner clip is geometry the reference
                // builds for every format; it is charged like a frame clip's,
                // once per target, so the ledger agrees across formats and
                // replays.
                if let Some(clip) = &image.corner_clip {
                    let geometry_segments =
                        u64::try_from(clip.path.operations.len()).unwrap_or(u64::MAX);
                    self.transact(
                        request,
                        ResourceContext::new(
                            ResourcePhase::SharedSemanticResolution,
                            "image_corner_clip_geometry",
                            image.source.pointer(),
                        )
                        .with_element_id(Some(image.source.id()))
                        .with_local_owner(Some(image.source.id())),
                        ResourceDelta {
                            geometry_segments,
                            total_work_units: geometry_segments.saturating_mul(32),
                            ..ResourceDelta::default()
                        },
                    )?;
                }
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
                    .saturating_add(text.text.fallback_probes.saturating_mul(32))
                    .saturating_add(geometry_segments.saturating_mul(32));
                let total_work_units = text_scalars
                    .saturating_mul(32)
                    .saturating_add(text.text.prepaid_glyphs.saturating_mul(32))
                    .saturating_add(text.text.prepaid_line_break_steps.saturating_mul(32))
                    .saturating_add(remaining_work_units);
                text.base_element_work_units = total_work_units;
                let path_headers = text
                    .text
                    .run_count()
                    .saturating_add(u64::try_from(text.frame_clips.len()).unwrap_or(u64::MAX));
                let working_bytes = glyphs
                    .saturating_mul(64)
                    .saturating_add(line_break_steps.saturating_mul(32))
                    .saturating_add(path_headers.saturating_mul(128))
                    .saturating_add(text.text.fallback_resource_count().saturating_mul(256))
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
                    self.commit_local_rejection_observation(
                        request,
                        context,
                        ResourceDelta {
                            geometry_segments,
                            ..ResourceDelta::default()
                        },
                    )?;
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
                if text.text.noto_emoji_substituted {
                    let diagnostic = fallback_font_diagnostic(
                        DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED,
                        DiagnosticSeverity::Info,
                        request.format(),
                        Some(text.source.id()),
                        text.source.pointer(),
                        None,
                    )
                    .map_err(|_| {
                        self.memory_failure(
                            request,
                            "compatibility_report",
                            "text_emoji_font_substitution",
                        )
                    })?;
                    self.retain_diagnostic_with_owner(
                        request,
                        ResourcePhase::SharedSemanticResolution,
                        "text_emoji_font_substitution",
                        diagnostic,
                        DiagnosticOwnership {
                            local_owner: Some(text.source.id()),
                            source_order: Some(text.source_order),
                            source_position: Some(0),
                        },
                    )?;
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
                let outline_segments = frame.outline.as_ref().map_or(0, |outline| {
                    u64::try_from(outline.path.operations.len()).unwrap_or(u64::MAX)
                });
                let clip_segments = frame.frame_clips.iter().fold(0_u64, |total, clip| {
                    total.saturating_add(
                        u64::try_from(clip.path.operations.len()).unwrap_or(u64::MAX),
                    )
                });
                let label_segments = frame.name.as_ref().map_or(0, |name| {
                    u64::try_from(name.label.path.operations.len()).unwrap_or(u64::MAX)
                });
                let text_scalars = frame.name.as_ref().map_or(0, |name| {
                    u64::try_from(name.label.text.chars().count()).unwrap_or(u64::MAX)
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
            let mut geometry_segments = fallback
                .geometry_command_count()
                .saturating_add(clip_segments);
            let mut glyphs = fallback.label.as_ref().map_or(0, |label| label.glyph_count);
            let mut working_bytes = 128_u64.saturating_add(geometry_segments.saturating_mul(64));
            let mut total_work_units = geometry_segments
                .saturating_mul(32)
                .saturating_add(glyphs.saturating_mul(32));
            let mut delta = ResourceDelta {
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
                self.commit_local_rejection_observation(
                    request,
                    context,
                    ResourceDelta {
                        geometry_segments,
                        ..ResourceDelta::default()
                    },
                )?;
                if self.has_resource_fallback_origin(fallback) && fallback.label.is_some() {
                    fallback.label = None;
                    geometry_segments = fallback
                        .geometry_command_count()
                        .saturating_add(clip_segments);
                    glyphs = 0;
                    working_bytes = 128_u64.saturating_add(geometry_segments.saturating_mul(64));
                    total_work_units = geometry_segments.saturating_mul(32);
                    if geometry_segments
                        <= u64::from(
                            request
                                .resource_limits()
                                .max_geometry_segments_per_element(),
                        )
                        && total_work_units <= request.resource_limits().max_element_work_units()
                    {
                        delta = ResourceDelta {
                            geometry_segments,
                            working_bytes,
                            total_work_units,
                            ..ResourceDelta::default()
                        };
                    } else if self.omit_failed_resource_fallback(fallback) {
                        self.charge_local_omission(
                            request,
                            fallback.source.id.as_deref(),
                            &fallback.source.pointer,
                            "fallback_local_omission",
                        )?;
                        fallback.render = false;
                        continue;
                    }
                } else if self.omit_failed_resource_fallback(fallback) {
                    self.charge_local_omission(
                        request,
                        fallback.source.id.as_deref(),
                        &fallback.source.pointer,
                        "fallback_local_omission",
                    )?;
                    fallback.render = false;
                    continue;
                } else {
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
            let delta = ResourceDelta {
                rasterized_pixels: pixels,
                total_work_units: pixels,
                ..ResourceDelta::default()
            };
            let context = ResourceContext::new(
                ResourcePhase::ProjectionAndFallback,
                "pdf_raster_fallback",
                &closure.json_pointer,
            )
            .with_element_id(Some(&closure.element_id))
            .with_local_owner(Some(&closure.element_id));
            if self.meter.would_cross_fatal(delta) {
                self.transact(request, context, delta)?;
                return Err(self.internal_failure(
                    request.format(),
                    "fatal PDF fallback preflight committed unexpectedly",
                    "/elements",
                ));
            }
            if closure.pixel_width > u64::from(limits.max_raster_dimension())
                || closure.pixel_height > u64::from(limits.max_raster_dimension())
            {
                self.preflight_local_rejection(request, context, delta)?;
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
                self.preflight_local_rejection(request, context, delta)?;
                return Err(self.pdf_raster_surface_failure(
                    request,
                    closure,
                    "max_raster_surface_pixels",
                    limits.max_raster_surface_pixels(),
                    pixels,
                    ValueKind::Pixels,
                ));
            }
            self.transact(request, context, delta)?;
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

    fn required_raster_surface_failure(
        &mut self,
        request: &RenderRequest,
        configured: u64,
        attempted: u64,
    ) -> RenderError {
        RenderError {
            category: RenderErrorCategory::ResourceLimit,
            code: ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED,
            message: Box::from("required PNG raster surface exceeds its configured limit"),
            format: request.format(),
            element_id: None,
            json_pointer: Box::from(""),
            details: Some(Box::new(ErrorDetails::ResourceLimit {
                field: "max_raster_surface_pixels",
                configured: ResourceValue::Integer(configured),
                value_kind: ValueKind::Pixels,
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
        let total_work_units =
            total_work_units.saturating_add(label.fallback_probes.saturating_mul(32));
        let working_bytes = glyphs
            .saturating_mul(64)
            .saturating_add(line_break_steps.saturating_mul(32))
            .saturating_add(label.run_count().saturating_mul(128))
            .saturating_add(label.fallback_resource_count().saturating_mul(256))
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
            self.commit_local_rejection_observation(
                request,
                context,
                ResourceDelta {
                    geometry_segments,
                    ..ResourceDelta::default()
                },
            )?;
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
        if label.noto_emoji_substituted {
            let diagnostic = fallback_font_diagnostic(
                DiagnosticCode::FONT_NOTO_EMOJI_SUBSTITUTED,
                DiagnosticSeverity::Info,
                request.format(),
                Some(outline.source.id()),
                outline.source.pointer(),
                None,
            )
            .map_err(|_| {
                self.memory_failure(
                    request,
                    "compatibility_report",
                    "embeddable_label_emoji_font_substitution",
                )
            })?;
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::SharedSemanticResolution,
                "embeddable_label_emoji_font_substitution",
                diagnostic,
                DiagnosticOwnership {
                    local_owner: Some(outline.source.id()),
                    source_order: Some(outline.source_order),
                    source_position: Some(0),
                },
            )?;
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
        fallbacks: &mut Vec<crate::scene_surface::SurfaceFallback>,
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
        let prospective_glyphs = text::prospective_glyph_count(&text.text);
        let stored_line_steps = text::line_count(&text.text);
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
            self.commit_local_rejection_observation(
                request,
                context,
                ResourceDelta {
                    text_scalars,
                    ..ResourceDelta::default()
                },
            )?;
            let replacement = if source.width > 0.0 && source.height > 0.0 {
                let fallback = known_element_fallback(source).map_err(|_| {
                    self.memory_failure(request, "semantic_geometry", "known_element_fallback")
                })?;
                fallback_fits_local_limits(&fallback, request.resource_limits()).then_some(fallback)
            } else {
                None
            };
            let can_replace = replacement.is_some();
            let diagnostic = crossing
                .diagnostic_with_recovery(
                    request.format(),
                    Some(source.source.id()),
                    source.source.pointer(),
                    if can_replace {
                        Recovery::Placeholder
                    } else {
                        Recovery::Omitted
                    },
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
                    "text_preflight_local_omission",
                )?;
            }
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

    fn omit_failed_resource_fallback(&mut self, fallback: &ResolvedFallback) -> bool {
        let original = self.report.diagnostics.iter_mut().find(|diagnostic| {
            diagnostic.element_id() == fallback.source.id.as_deref()
                && diagnostic.json_pointer() == fallback.source.pointer
                && matches!(
                    diagnostic.code(),
                    DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
                        | DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
                        | DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED
                )
                && matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::ResourceLimit {
                        recovery: Some(Recovery::Placeholder),
                        ..
                    })
                )
        });
        let Some(original) = original else {
            return false;
        };
        let Some(details) = original.details.as_mut() else {
            return false;
        };
        let ErrorDetails::ResourceLimit { recovery, .. } = details.as_mut() else {
            return false;
        };
        *recovery = Some(Recovery::Omitted);
        true
    }

    fn has_resource_fallback_origin(&self, fallback: &ResolvedFallback) -> bool {
        self.report.diagnostics.iter().any(|diagnostic| {
            diagnostic.element_id() == fallback.source.id.as_deref()
                && diagnostic.json_pointer() == fallback.source.pointer
                && matches!(
                    diagnostic.code(),
                    DiagnosticCode::RESOURCE_ELEMENT_COMPLEXITY_REPLACED
                        | DiagnosticCode::RESOURCE_ELEMENT_WORK_EXCEEDED
                        | DiagnosticCode::RESOURCE_COORDINATE_RANGE_EXCEEDED
                )
                && matches!(
                    diagnostic.details(),
                    Some(ErrorDetails::ResourceLimit {
                        recovery: Some(Recovery::Placeholder),
                        ..
                    })
                )
        })
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
                self.commit_local_rejection_observation(
                    request,
                    context,
                    ResourceDelta {
                        text_scalars,
                        ..ResourceDelta::default()
                    },
                )?;
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
                        "compatibility fallback label omitted because it exceeds the text limit",
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
                fallbacks[index].label.clear();
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
        diagnostic.order = DiagnosticOrder::retained(
            ownership.source_order.or_else(|| {
                element_owner_index(&diagnostic.json_pointer)
                    .map(|index| self.element_source_order_base.saturating_add(index))
            }),
            ownership.source_position,
            u64::try_from(self.report.diagnostics.len()).unwrap_or(u64::MAX),
            self.diagnostic_attribution,
        );
        self.report.diagnostics.push(diagnostic);
        Ok(())
    }

    fn scope_frame_diagnostics(&mut self, participating: &[usize]) {
        self.report.diagnostics.retain_mut(|diagnostic| {
            let Some(source_index) = element_owner_index(&diagnostic.json_pointer) else {
                return true;
            };
            let Ok(source_index) = usize::try_from(source_index) else {
                return false;
            };
            if participating.binary_search(&source_index).is_err() {
                return false;
            }
            diagnostic.order.attribution = DiagnosticAttribution::Target;
            true
        });
    }

    fn flush_frame_normalization_diagnostics(
        &mut self,
        request: &RenderRequest,
        participating: &[usize],
        selected_file_ids: &[String],
    ) -> Result<(), RenderError> {
        let pending = std::mem::take(&mut self.pending_frame_normalization_diagnostics);
        for pending in pending {
            let element_source = element_owner_index(&pending.diagnostic.json_pointer)
                .and_then(|source_index| usize::try_from(source_index).ok());
            if element_source
                .is_some_and(|source_index| participating.binary_search(&source_index).is_err())
            {
                continue;
            }
            if pending.diagnostic.json_pointer.starts_with("/files/")
                && !selected_file_ids.iter().any(|file_id| {
                    crate::scene::pointer_child("/files", file_id)
                        .is_ok_and(|pointer| pending.diagnostic.json_pointer.starts_with(&pointer))
                })
            {
                continue;
            }
            self.diagnostic_attribution = if element_source.is_some()
                || pending.diagnostic.json_pointer.starts_with("/files/")
            {
                DiagnosticAttribution::Target
            } else {
                DiagnosticAttribution::SceneGlobal
            };
            self.retain_diagnostic_with_owner(
                request,
                ResourcePhase::JsonTraversalAndNormalization,
                "normalization_repair",
                pending.diagnostic,
                DiagnosticOwnership {
                    local_owner: pending.local_owner.as_deref(),
                    source_order: Some(pending.source_order),
                    source_position: Some(pending.source_position),
                },
            )?;
        }
        Ok(())
    }

    fn take_report(&mut self) -> CompatibilityReport {
        sort_diagnostics(&mut self.report.diagnostics);
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

        let emitted = encode().map_err(|error| self.format_failure(request, error_code, error))?;
        if emitted != measured {
            return Err(self.internal_failure(
                request.format(),
                "measured and emitted artifact bytes differ",
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

    fn commit_local_rejection_observation(
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
        match self
            .meter
            .commit_local_rejection_observation(context, delta)
        {
            Ok(()) => Ok(()),
            Err(crossing) => Err(self.resource_failure(request, crossing, failure_location)),
        }
    }

    fn preflight_local_rejection(
        &mut self,
        request: &RenderRequest,
        context: ResourceContext<'_>,
        delta: ResourceDelta,
    ) -> Result<(), RenderError> {
        let failure_location = context
            .try_failure_location()
            .map_err(|_| self.memory_failure(request, "resource_failure", "source_location"))?;
        match self.meter.preflight_local_rejection(context, delta) {
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
            FormatEncodeError::Metadata(_) => self.encoding_failure(
                request.format(),
                code,
                "mandatory artifact metadata could not be encoded".to_owned(),
            ),
            FormatEncodeError::SvgFontSubset {
                stage,
                resolved_family,
                asset_identity,
            } => RenderError {
                category: RenderErrorCategory::Encoding,
                code: ErrorCode::ENCODE_SVG_FONT_SUBSET_FAILED,
                message: Box::from("a required SVG font subset could not be produced"),
                format: request.format(),
                element_id: None,
                json_pointer: Box::from(""),
                details: Some(Box::new(ErrorDetails::SvgFontSubsetFailure {
                    stage,
                    resolved_family: resolved_family.to_owned(),
                    asset_identity: asset_identity.to_owned(),
                })),
                report: self.take_report(),
            },
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
