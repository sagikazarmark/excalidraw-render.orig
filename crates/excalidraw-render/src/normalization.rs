mod fractional_index;
mod kinds;
mod planning;

use kinds::{element_fields, normalize_element_kind, normalized_roundness};
pub(crate) use planning::NORMALIZATION_WORK_UNITS;
use planning::{
    kind_working_bytes, normalized_working_bytes, owned_field_bytes, planned_kind_working_bytes,
    GENERATED_ID_BYTES,
};

use std::collections::TryReserveError;

use sha2::{Digest, Sha256};

use crate::outcome::DiagnosticOrder;
use crate::scene::{
    pointer_child, pointer_index, ElementKind, ElementSource, FrameRenderingSource, SourceField,
};
use crate::{
    CompatibilityDiagnostic, CompatibilityRevision, DiagnosticCode, DiagnosticEffect,
    DiagnosticSeverity, ErrorDetails, OutputFormat, Recovery, SceneDocument, SupplementalAssets,
};

const MISSING_ID_DOMAIN: &[u8] = b"excalidraw-render:missing-element-id:v1\0";
const DUPLICATE_ID_DOMAIN: &[u8] = b"excalidraw-render:duplicate-element-id:v1\0";
const ID_COLLISION_DOMAIN: &[u8] = b"excalidraw-render:element-id-collision:v1\0";
const ROUGH_SEED_DOMAIN: &[u8] = b"excalidraw-render:rough-seed:v1\0";

#[derive(Clone, Debug)]
pub(crate) struct NormalizedElement {
    pub(crate) source: NormalizedElementSource,
    pub(crate) source_order: u64,
    pub(crate) index: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) angle: f64,
    pub(crate) stroke_color: String,
    pub(crate) background_color: String,
    pub(crate) fill_style: String,
    pub(crate) stroke_width: f64,
    pub(crate) stroke_style: String,
    pub(crate) roughness: f64,
    pub(crate) opacity: f64,
    pub(crate) link: Option<String>,
    pub(crate) frame_id: Option<String>,
    pub(crate) seed: i32,
    pub(crate) roundness: Option<NormalizedRoundness>,
    pub(crate) normalized_working_bytes: u64,
    pub(crate) normalization_work_units: u64,
    pub(crate) repair_work_units: u64,
    pub(crate) kind_name: &'static str,
    pub(crate) kind: NormalizedElementKind,
    pub(crate) direct_outline_compatible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NormalizedElementKind {
    Rectangle,
    Diamond,
    Ellipse,
    Text(NormalizedText),
    Linear(NormalizedLinear),
    Freedraw(NormalizedFreedraw),
    Image(NormalizedImage),
    Frame(NormalizedFrame),
    MagicFrame(NormalizedFrame),
    Embeddable { iframe: bool, label: String },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedText {
    pub(crate) font_size: f64,
    pub(crate) font_family: i32,
    pub(crate) text: String,
    pub(crate) original_text: String,
    pub(crate) text_align: String,
    pub(crate) vertical_align: String,
    pub(crate) line_height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NormalizedRoundness {
    Proportional,
    Adaptive { radius: f64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NormalizedLinearKind {
    Line,
    Arrow,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedLinear {
    pub(crate) kind: NormalizedLinearKind,
    pub(crate) points: Vec<[f64; 2]>,
    pub(crate) start_binding: Option<NormalizedBinding>,
    pub(crate) end_binding: Option<NormalizedBinding>,
    pub(crate) last_committed_point: Option<[f64; 2]>,
    pub(crate) start_arrowhead: Option<String>,
    pub(crate) end_arrowhead: Option<String>,
    pub(crate) elbowed: bool,
    pub(crate) geometry_available: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedBinding {
    pub(crate) element_id: String,
    pub(crate) focus: f64,
    pub(crate) gap: f64,
    pub(crate) fixed_point: Option<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedFreedraw {
    pub(crate) points: Vec<[f64; 2]>,
    pub(crate) pressures: Vec<f64>,
    pub(crate) simulate_pressure: bool,
    pub(crate) last_committed_point: Option<[f64; 2]>,
    pub(crate) geometry_available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NormalizedImageStatus {
    Pending,
    Saved,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedImage {
    pub(crate) file_id: Option<String>,
    pub(crate) status: NormalizedImageStatus,
    pub(crate) scale: [i8; 2],
    pub(crate) crop: Option<NormalizedImageCrop>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NormalizedImageCrop {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) natural_width: f64,
    pub(crate) natural_height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedFrame {
    pub(crate) name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NormalizedFrameRendering {
    pub(crate) enabled: bool,
    pub(crate) clip: bool,
    pub(crate) name: bool,
    pub(crate) outline: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalizedFileRecord {
    pub(crate) key: String,
    pub(crate) id: String,
    pub(crate) created: Option<u64>,
    pub(crate) source_pointer: String,
    pub(crate) source_order: u64,
    pub(crate) normalized_working_bytes: u64,
    normalization_work_units: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct NormalizedElementSource {
    source_index: usize,
    id: String,
    pointer: String,
    coordinate_positions: [usize; 4],
}

impl NormalizedElementSource {
    pub(crate) const fn source_index(&self) -> usize {
        self.source_index
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn pointer(&self) -> &str {
        &self.pointer
    }

    /// The source position of one coordinate member in the total diagnostic
    /// order, packed the way every other element diagnostic packs its member
    /// position so a resource crossing on `width` sorts after a repair on `y`.
    pub(crate) fn coordinate_position(&self, field: &str) -> u64 {
        let position = match field {
            "x" => self.coordinate_positions[0],
            "y" => self.coordinate_positions[1],
            "width" => self.coordinate_positions[2],
            "height" => self.coordinate_positions[3],
            _ => usize::MAX,
        };
        SourceDiagnosticPosition::top(position).as_u64()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct NormalizationOutcome {
    pub(crate) elements: Vec<NormalizedElement>,
    pub(crate) fallbacks: Vec<crate::scene_surface::SurfaceFallback>,
    pub(crate) view_background_color: String,
    pub(crate) view_background_working_bytes: u64,
    pub(crate) frame_rendering: NormalizedFrameRendering,
    pub(crate) files: Vec<NormalizedFileRecord>,
}

pub(crate) enum NormalizationEvent<'a> {
    PreflightRecord {
        event: &'static str,
        json_pointer: &'a str,
        working_bytes: u64,
        work_units: u64,
    },
    CommitRecord {
        event: &'static str,
        element_id: Option<&'a str>,
        local_owner: Option<&'a str>,
        json_pointer: &'a str,
        working_bytes: u64,
        work_units: u64,
    },
    Diagnostic {
        diagnostic: CompatibilityDiagnostic,
        local_owner: Option<&'a str>,
        source_order: u64,
        source_position: u64,
    },
}

struct PendingNormalizationEvent {
    source_order: u64,
    ordinal: u64,
    kind: PendingNormalizationEventKind,
}

enum PendingNormalizationEventKind {
    Record {
        event: &'static str,
        preflighted: bool,
        element_id: Option<String>,
        local_owner: Option<String>,
        json_pointer: String,
        working_bytes: u64,
        work_units: u64,
    },
    Diagnostic {
        diagnostic: CompatibilityDiagnostic,
        local_owner: Option<String>,
        source_position: u64,
    },
}

#[derive(Debug)]
pub(crate) enum NormalizationError<E> {
    Observer(E),
    MemoryUnavailable { source_index: Option<usize> },
}

impl<E> From<TryReserveError> for NormalizationError<E> {
    fn from(_: TryReserveError) -> Self {
        Self::MemoryUnavailable { source_index: None }
    }
}

struct CommonFields<'a> {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
    stroke_color: &'a str,
    background_color: &'a str,
    fill_style: &'a str,
    stroke_width: f64,
    stroke_style: &'a str,
    roughness: f64,
    opacity: f64,
    source_seed: i32,
}

enum IdentityPlan<'a> {
    Retained(&'a str),
    Duplicate(&'a str),
    Missing,
    Invalid,
}

struct PendingElement<'a> {
    source_index: usize,
    source: ElementSource<'a>,
    fields: CommonFields<'a>,
    direct_outline_compatible: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceDiagnosticPosition {
    member: usize,
    nested: usize,
}

impl SourceDiagnosticPosition {
    const fn top(member: usize) -> Self {
        Self { member, nested: 0 }
    }

    const fn nested(member: usize, nested: usize) -> Self {
        Self {
            member,
            nested: nested.saturating_add(1),
        }
    }

    /// Packs the member position into the high half and the nested ordinal
    /// into the low half, so a diagnostic sorts by the source member it
    /// belongs to before its position inside that member. A missing nested
    /// member is spelled with a saturated nested ordinal; clamping it to the
    /// low half keeps it last within its own member rather than after every
    /// later member of the element.
    fn as_u64(self) -> u64 {
        let nested = u64::try_from(self.nested)
            .unwrap_or(u64::MAX)
            .min(u64::from(u32::MAX));
        u64::try_from(self.member)
            .unwrap_or(u64::MAX)
            .saturating_mul(u64::from(u32::MAX) + 1)
            .saturating_add(nested)
    }
}

fn nested_path_ordinal(first: usize, second: usize) -> usize {
    first
        .saturating_add(1)
        .saturating_mul(usize::from(u16::MAX) + 1)
        .saturating_add(second.saturating_add(1))
}

type ElementDiagnostic = (SourceDiagnosticPosition, CompatibilityDiagnostic);

struct ElementDiagnosticContext<'a, 'scene> {
    diagnostics: &'a mut Vec<ElementDiagnostic>,
    source: &'a ElementSource<'scene>,
    element_pointer: &'a str,
    element_id: &'a str,
    format: OutputFormat,
    policy: &'static crate::scene_surface::SceneSurfacePolicy,
    diagnostic_limit: usize,
}

#[derive(Clone, Copy)]
struct NormalizationParticipation {
    authoritative_file: bool,
    frame_names: bool,
}

impl ElementDiagnosticContext<'_, '_> {
    fn can_retain(&self) -> bool {
        self.diagnostics.len() < self.diagnostic_limit
    }

    fn unsupported(
        &mut self,
        field: &str,
        effects: &[DiagnosticEffect],
        recovery: Recovery,
        recipe_id: Option<&str>,
    ) -> Result<(), TryReserveError> {
        if !self.can_retain() {
            return Ok(());
        }
        let pointer = pointer_child(self.element_pointer, field)?;
        let semantic_path = pointer_child("", field)?;
        let manifested = crate::scene_surface::manifested_element_field(
            self.policy,
            self.source.kind.scene_surface_kind(),
            &semantic_path,
        );
        let manifested_effects = manifested
            .map(|field| manifested_effects(field.invalid.effects))
            .transpose()?;
        let code = manifested
            .and_then(|field| field.invalid.code)
            .map(DiagnosticCode::manifested)
            .unwrap_or(DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE);
        let recovery = manifested
            .and_then(|field| field.invalid.recovery)
            .and_then(manifested_recovery)
            .unwrap_or(recovery);
        let recipe_id = manifested
            .and_then(|field| field.invalid.recipe)
            .or(recipe_id);
        let diagnostic = compatibility_diagnostic(
            code,
            self.format,
            manifested_effects.as_deref().unwrap_or(effects),
            Some(self.element_id),
            &pointer,
            "unsupported property value used its revision-1 recovery",
            DiagnosticRecovery::new(recovery, recipe_id),
        )?;
        self.diagnostics.try_reserve(1)?;
        self.diagnostics.push((
            SourceDiagnosticPosition::top(self.source.member_position(field).unwrap_or(usize::MAX)),
            diagnostic,
        ));
        Ok(())
    }

    fn unsupported_nested(
        &mut self,
        field: &str,
        nested_field: &str,
        nested_position: Option<usize>,
        component: Option<usize>,
        effects: &[DiagnosticEffect],
        recovery: Recovery,
    ) -> Result<(), TryReserveError> {
        if !self.can_retain() {
            return Ok(());
        }
        let mut pointer =
            pointer_child(&pointer_child(self.element_pointer, field)?, nested_field)?;
        if let Some(component) = component {
            pointer = pointer_index(&pointer, component)?;
        }
        let diagnostic = compatibility_diagnostic(
            DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
            self.format,
            effects,
            Some(self.element_id),
            &pointer,
            "unsupported nested property value used its revision-1 recovery",
            DiagnosticRecovery::new(recovery, None),
        )?;
        self.diagnostics.try_reserve(1)?;
        self.diagnostics.push((
            SourceDiagnosticPosition::nested(
                self.source.member_position(field).unwrap_or(usize::MAX),
                nested_position.unwrap_or(usize::MAX - 1),
            ),
            diagnostic,
        ));
        Ok(())
    }

    fn unsupported_property_nested(
        &mut self,
        field: &str,
        nested_field: &str,
        nested_position: usize,
    ) -> Result<(), TryReserveError> {
        if !self.can_retain() {
            return Ok(());
        }
        let pointer = pointer_child(&pointer_child(self.element_pointer, field)?, nested_field)?;
        let diagnostic = compatibility_diagnostic(
            DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
            self.format,
            &[
                DiagnosticEffect::Visual,
                DiagnosticEffect::Semantic,
                DiagnosticEffect::Metadata,
            ],
            Some(self.element_id),
            &pointer,
            "unmatched nested property was ignored by the revision-1 scene-surface policy",
            DiagnosticRecovery::new(Recovery::Ignored, None),
        )?;
        self.diagnostics.try_reserve(1)?;
        self.diagnostics.push((
            SourceDiagnosticPosition::nested(
                self.source.member_position(field).unwrap_or(usize::MAX),
                nested_position,
            ),
            diagnostic,
        ));
        Ok(())
    }

    fn unsupported_property_at(
        &mut self,
        field: &str,
        nested_position: usize,
        pointer: &str,
    ) -> Result<(), TryReserveError> {
        if !self.can_retain() {
            return Ok(());
        }
        let diagnostic = compatibility_diagnostic(
            DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
            self.format,
            &[
                DiagnosticEffect::Visual,
                DiagnosticEffect::Semantic,
                DiagnosticEffect::Metadata,
            ],
            Some(self.element_id),
            pointer,
            "unmatched nested property was ignored by the revision-1 scene-surface policy",
            DiagnosticRecovery::new(Recovery::Ignored, None),
        )?;
        self.diagnostics.try_reserve(1)?;
        self.diagnostics.push((
            SourceDiagnosticPosition::nested(
                self.source.member_position(field).unwrap_or(usize::MAX),
                nested_position,
            ),
            diagnostic,
        ));
        Ok(())
    }

    fn missing_image_file(
        &mut self,
        file_id: Option<&str>,
        status: NormalizedImageStatus,
    ) -> Result<(), TryReserveError> {
        if !self.can_retain() {
            return Ok(());
        }
        let pointer = pointer_child(self.element_pointer, "fileId")?;
        let (recovery, recipe_id) = match self.format {
            OutputFormat::Png => (
                Recovery::Placeholder,
                Some(match status {
                    NormalizedImageStatus::Error => "official.image-error@sha256:f3c63c8600147961d006c0eea92ea6aff87bdc54711a2368af895a78154919e9",
                    NormalizedImageStatus::Pending | NormalizedImageStatus::Saved => "official.image@sha256:0a04dbba7ed6a0ce33aa61d9abf02b13685715241022483139b28eb666b8219d",
                }),
            ),
            OutputFormat::Svg | OutputFormat::Pdf => (Recovery::Omitted, None),
        };
        let asset_id = file_id.unwrap_or("");
        let diagnostic = CompatibilityDiagnostic {
            code: DiagnosticCode::IMAGE_MISSING_FILE,
            severity: DiagnosticSeverity::Warning,
            format: self.format,
            effects: manifested_effects(&["visual"])?,
            element_id: Some(try_owned(self.element_id)?),
            json_pointer: pointer,
            message: try_owned("image reference had no authoritative file payload")?,
            details: Some(Box::new(ErrorDetails::Asset {
                asset_id: try_owned(asset_id)?,
                claimed_media_type: None,
                validated_media_type: None,
                recovery,
                recipe_id: recipe_id.map(try_owned).transpose()?,
            })),
            order: DiagnosticOrder::default(),
        };
        self.diagnostics.try_reserve(1)?;
        self.diagnostics.push((
            SourceDiagnosticPosition::top(
                self.source.member_position("fileId").unwrap_or(usize::MAX),
            ),
            diagnostic,
        ));
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn normalize_scene<E>(
    scene: &SceneDocument,
    revision: CompatibilityRevision,
    format: OutputFormat,
    observe: impl FnMut(NormalizationEvent<'_>) -> Result<(), E>,
) -> Result<Option<NormalizationOutcome>, NormalizationError<E>> {
    let Some(policy) =
        crate::catalog::for_revision(revision).map(|catalog| catalog.scene_surface())
    else {
        return Ok(None);
    };
    normalize_scene_impl(
        scene,
        revision,
        policy,
        format,
        true,
        usize::MAX,
        |_| false,
        observe,
    )
}

/// The diagnostics revision-1 normalization emits for `scene`, in the order
/// the session's final report lists them, with the normalized outcome. The
/// session retains each normalization diagnostic under the source owner and
/// member position of the event that carried it (`DiagnosticOrder::retained`)
/// and sorts the report (`render::sort_diagnostics`); this does the same, so a
/// test at this seam asserts the order a render would report for diagnostics
/// normalization alone produces. `frame_names` is the request's frame-name
/// control as the session derives it; `supplemental_assets` decides which file
/// records have a supplemental payload.
#[cfg(test)]
pub(crate) fn reported_diagnostics(
    scene: &SceneDocument,
    format: OutputFormat,
    frame_names: bool,
    supplemental_assets: &SupplementalAssets,
) -> (Vec<CompatibilityDiagnostic>, NormalizationOutcome) {
    use crate::outcome::DiagnosticAttribution;

    let policy = crate::catalog::for_revision(CompatibilityRevision::REVISION_1)
        .expect("revision 1 is cataloged")
        .scene_surface();
    let mut diagnostics: Vec<CompatibilityDiagnostic> = Vec::new();
    let normalized = normalize_scene_with_assets(
        scene,
        CompatibilityRevision::REVISION_1,
        policy,
        format,
        supplemental_assets,
        frame_names,
        usize::MAX,
        |event| {
            if let NormalizationEvent::Diagnostic {
                mut diagnostic,
                source_order,
                source_position,
                ..
            } = event
            {
                diagnostic.order = DiagnosticOrder::retained(
                    Some(source_order),
                    Some(source_position),
                    u64::try_from(diagnostics.len()).unwrap_or(u64::MAX),
                    DiagnosticAttribution::SceneGlobal,
                );
                diagnostics.push(diagnostic);
            }
            Ok::<_, ()>(())
        },
    )
    .expect("normalization succeeds")
    .expect("the scene is in the implemented normalization slice");
    crate::render::sort_diagnostics(&mut diagnostics);
    (diagnostics, normalized)
}

/// `(code, pointer)` per diagnostic: what most assertions on a report compare.
#[cfg(test)]
pub(crate) fn codes_and_pointers(
    diagnostics: &[CompatibilityDiagnostic],
) -> Vec<(DiagnosticCode, &str)> {
    diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer()))
        .collect()
}

/// The pointer of each diagnostic, in report order.
#[cfg(test)]
pub(crate) fn pointers(diagnostics: &[CompatibilityDiagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(CompatibilityDiagnostic::json_pointer)
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn normalize_scene_with_assets<E>(
    scene: &SceneDocument,
    revision: CompatibilityRevision,
    policy: &'static crate::scene_surface::SceneSurfacePolicy,
    format: OutputFormat,
    supplemental_assets: &SupplementalAssets,
    frame_names: bool,
    surface_diagnostic_limit: usize,
    observe: impl FnMut(NormalizationEvent<'_>) -> Result<(), E>,
) -> Result<Option<NormalizationOutcome>, NormalizationError<E>> {
    normalize_scene_impl(
        scene,
        revision,
        policy,
        format,
        frame_names,
        surface_diagnostic_limit,
        |file_id| supplemental_assets.get(file_id).is_some(),
        observe,
    )
}

#[allow(clippy::too_many_arguments)]
fn normalize_scene_impl<E>(
    scene: &SceneDocument,
    revision: CompatibilityRevision,
    policy: &'static crate::scene_surface::SceneSurfacePolicy,
    format: OutputFormat,
    requested_frame_names: bool,
    surface_diagnostic_limit: usize,
    has_supplemental_file: impl Fn(&str) -> bool,
    mut observe: impl FnMut(NormalizationEvent<'_>) -> Result<(), E>,
) -> Result<Option<NormalizationOutcome>, NormalizationError<E>> {
    let frame_names = requested_frame_names;
    let view_background_color = manifested_value(
        scene.view_background_color(),
        "#ffffff",
        crate::scene_surface::css_color_is_admitted,
    );
    let view_background_working_bytes = owned_field_bytes(view_background_color.len());
    let element_count = scene.element_occurrence_count();
    let mut pending_events = Vec::new();
    pending_events.try_reserve(element_count.saturating_add(2))?;
    let surface = crate::scene_surface::classify(
        scene,
        policy,
        format,
        requested_frame_names,
        surface_diagnostic_limit,
    )?;
    let mut planned_diagnostic_count = surface.diagnostics.len();
    for surface_diagnostic in surface.diagnostics {
        push_pending_event(
            &mut pending_events,
            surface_diagnostic.source_order,
            PendingNormalizationEventKind::Diagnostic {
                diagnostic: surface_diagnostic.diagnostic,
                local_owner: surface_diagnostic.local_owner,
                source_position: surface_diagnostic.source_position,
            },
        )?;
    }
    let mut pending = Vec::new();
    pending.try_reserve_exact(element_count)?;

    for index in 0..element_count {
        if scene.element_is_silent(index) {
            continue;
        }
        let Some(source) = scene.element_at(index) else {
            continue;
        };
        let is_deleted = match source.is_deleted {
            SourceField::Value(value) => value,
            SourceField::Missing | SourceField::Invalid => false,
        };
        if is_deleted {
            continue;
        }
        let Some(fields) = element_fields(&source, policy) else {
            return Ok(None);
        };
        let direct_outline_compatible = matches!(
            source.kind,
            ElementKind::Rectangle
                | ElementKind::Diamond
                | ElementKind::Ellipse
                | ElementKind::Line
                | ElementKind::LegacyDraw
                | ElementKind::Freedraw
                | ElementKind::Arrow
                | ElementKind::Iframe
                | ElementKind::Embeddable
        ) && defaulted(source.group_ids, 0).is_some()
            && !matches!(source.bound_elements, SourceField::Invalid);
        pending.push(PendingElement {
            source_index: index,
            source,
            fields,
            direct_outline_compatible,
        });
    }

    let mut referenced_file_ids = Vec::new();
    referenced_file_ids.try_reserve_exact(pending.len())?;
    for element in &pending {
        if matches!(element.source.kind, ElementKind::Image) {
            if let SourceField::Value(Some(file_id)) = element.source.file_id {
                referenced_file_ids.push(file_id);
            }
        }
    }
    referenced_file_ids.sort_unstable();
    referenced_file_ids.dedup();
    let file_surface_diagnostics = crate::scene_surface::classify_referenced_files(
        scene,
        policy,
        &referenced_file_ids,
        format,
        surface_diagnostic_limit.saturating_sub(planned_diagnostic_count),
    )?;
    planned_diagnostic_count =
        planned_diagnostic_count.saturating_add(file_surface_diagnostics.len());
    for surface_diagnostic in file_surface_diagnostics {
        push_pending_event(
            &mut pending_events,
            surface_diagnostic.source_order,
            PendingNormalizationEventKind::Diagnostic {
                diagnostic: surface_diagnostic.diagnostic,
                local_owner: surface_diagnostic.local_owner,
                source_position: surface_diagnostic.source_position,
            },
        )?;
    }

    let mut scene_file_ids = Vec::new();
    scene_file_ids.try_reserve_exact(scene.file_record_count())?;
    for index in 0..scene.file_record_count() {
        if let Some(file) = scene.file_record_at(index) {
            scene_file_ids.push(file.key());
        }
    }
    scene_file_ids.sort_unstable();
    for file_id in &referenced_file_ids {
        if planned_diagnostic_count >= surface_diagnostic_limit {
            break;
        }
        if scene_file_ids.binary_search(file_id).is_ok() && has_supplemental_file(file_id) {
            if let Some(surface_diagnostic) =
                crate::scene_surface::supplement_conflict(scene, file_id, format)?
            {
                push_pending_event(
                    &mut pending_events,
                    surface_diagnostic.source_order,
                    PendingNormalizationEventKind::Diagnostic {
                        diagnostic: surface_diagnostic.diagnostic,
                        local_owner: surface_diagnostic.local_owner,
                        source_position: surface_diagnostic.source_position,
                    },
                )?;
                planned_diagnostic_count = planned_diagnostic_count.saturating_add(1);
            }
        }
    }

    let mut source_indices = Vec::new();
    source_indices.try_reserve_exact(pending.len())?;
    for element in &pending {
        source_indices.push(match element.source.index {
            SourceField::Value(Some(index)) if !index.is_empty() => Some(index),
            SourceField::Missing | SourceField::Value(None | Some(_)) | SourceField::Invalid => {
                None
            }
        });
    }
    if !fractional_index::are_strictly_ascending(&source_indices) {
        for index in &mut source_indices {
            if index.is_some_and(|index| !fractional_index::is_valid_key(index)) {
                *index = None;
            }
        }
    }
    let normalized_indices = fractional_index::repair(&source_indices)?;

    let mut elements = Vec::new();
    elements.try_reserve_exact(pending.len())?;
    let mut used_ids = Vec::<String>::new();

    for (pending, normalized_index) in pending.into_iter().zip(normalized_indices) {
        let index = pending.source_index;
        let source = pending.source;
        let fields = pending.fields;
        macro_rules! memory_at_source {
            ($result:expr) => {
                $result.map_err(|_| NormalizationError::MemoryUnavailable {
                    source_index: Some(index),
                })?
            };
        }
        let element_pointer = memory_at_source!(pointer_index("/elements", index));
        let participation = NormalizationParticipation {
            authoritative_file: matches!(
                source.file_id,
                SourceField::Value(Some(file_id))
                    if scene_file_ids.binary_search(&file_id).is_ok()
                        || has_supplemental_file(file_id)
            ),
            frame_names,
        };

        let identity_plan = match source.id {
            SourceField::Value("") | SourceField::Invalid => IdentityPlan::Invalid,
            SourceField::Value(id) if used_ids.iter().any(|used| used == id) => {
                IdentityPlan::Duplicate(id)
            }
            SourceField::Value(id) => IdentityPlan::Retained(id),
            SourceField::Missing => IdentityPlan::Missing,
        };
        let normalized_id_bytes = match identity_plan {
            IdentityPlan::Retained(id) => id.len(),
            IdentityPlan::Duplicate(_) | IdentityPlan::Missing | IdentityPlan::Invalid => {
                GENERATED_ID_BYTES
            }
        };
        let normalization_work_units = NORMALIZATION_WORK_UNITS.saturating_add(8).saturating_add(
            source
                .kind
                .normalized_field_count(participation)
                .saturating_mul(8),
        );
        let planned_working_bytes = normalized_working_bytes(normalized_id_bytes, &fields)
            .saturating_add(owned_field_bytes(normalized_index.len()))
            .saturating_add(memory_at_source!(planned_kind_working_bytes(
                &source,
                participation
            )));
        let normalization_event = source.kind.normalization_event_name();
        observe(NormalizationEvent::PreflightRecord {
            event: normalization_event,
            json_pointer: &element_pointer,
            working_bytes: planned_working_bytes,
            work_units: normalization_work_units,
        })
        .map_err(NormalizationError::Observer)?;

        let mut id_diagnostic = None;
        let id = match identity_plan {
            IdentityPlan::Retained(id) => memory_at_source!(try_owned(id)),
            IdentityPlan::Duplicate(original_id) => {
                let canonical_element = memory_at_source!(source.canonical_element_bytes());
                let id = memory_at_source!(generated_duplicate_element_id(
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    original_id,
                    &canonical_element,
                ));
                let id = memory_at_source!(unique_generated_id(
                    id,
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &used_ids,
                ));
                id_diagnostic = Some((
                    DiagnosticCode::NORMALIZATION_DUPLICATE_ELEMENT_ID,
                    "duplicate element identity was rekeyed deterministically",
                ));
                id
            }
            IdentityPlan::Missing => {
                let canonical_element = memory_at_source!(source.canonical_element_bytes());
                let id = memory_at_source!(generated_element_id(
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &canonical_element,
                ));
                let id = memory_at_source!(unique_generated_id(
                    id,
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &used_ids,
                ));
                id_diagnostic = Some((
                    DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID,
                    "missing element identity was repaired deterministically",
                ));
                id
            }
            IdentityPlan::Invalid => {
                let canonical_element = memory_at_source!(source.canonical_element_bytes());
                let id = memory_at_source!(generated_element_id(
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &canonical_element,
                ));
                let id = memory_at_source!(unique_generated_id(
                    id,
                    revision,
                    u64::try_from(index).unwrap_or(u64::MAX),
                    &used_ids,
                ));
                id_diagnostic = Some((
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "invalid element identity was repaired deterministically",
                ));
                id
            }
        };
        let element_diagnostic_limit =
            surface_diagnostic_limit.saturating_sub(planned_diagnostic_count);
        let mut element_diagnostics = Vec::new();
        memory_at_source!(element_diagnostics.try_reserve_exact(16));
        if element_diagnostics.len() < element_diagnostic_limit {
            if let Some((code, message)) = id_diagnostic {
                let pointer = memory_at_source!(pointer_child(&element_pointer, "id"));
                let diagnostic = memory_at_source!(repair_diagnostic(
                    code,
                    format,
                    &[DiagnosticEffect::InputRepair],
                    &id,
                    &pointer,
                    message,
                ));
                element_diagnostics.push((
                    SourceDiagnosticPosition::top(source.member_position("id").unwrap_or(0)),
                    diagnostic,
                ));
            }
        }

        let seed = if fields.source_seed == 0 {
            let seed = generated_rough_seed(revision, &id);
            if element_diagnostics.len() < element_diagnostic_limit {
                let pointer = memory_at_source!(pointer_child(&element_pointer, "seed"));
                let diagnostic = memory_at_source!(repair_diagnostic(
                    DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                    format,
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    &id,
                    &pointer,
                    "zero Rough seed was repaired deterministically",
                ));
                element_diagnostics.push((
                    SourceDiagnosticPosition::top(
                        source.member_position("seed").unwrap_or(usize::MAX - 1),
                    ),
                    diagnostic,
                ));
            }
            seed
        } else {
            fields.source_seed
        };

        if source.updated_is_missing && element_diagnostics.len() < element_diagnostic_limit {
            let pointer = memory_at_source!(pointer_child(&element_pointer, "updated"));
            let diagnostic = memory_at_source!(repair_diagnostic(
                DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                format,
                &[DiagnosticEffect::InputRepair],
                &id,
                &pointer,
                "missing required timestamp was repaired to canonical zero",
            ));
            element_diagnostics.push((SourceDiagnosticPosition::top(usize::MAX), diagnostic));
        }

        let (kind, x, y, width, height) = memory_at_source!(normalize_element_kind(
            &source,
            &fields,
            policy,
            &element_pointer,
            &id,
            format,
            participation,
            element_diagnostic_limit,
            &mut element_diagnostics,
        ));
        planned_diagnostic_count =
            planned_diagnostic_count.saturating_add(element_diagnostics.len());

        let actual_working_bytes = normalized_working_bytes(id.len(), &fields)
            .saturating_add(owned_field_bytes(normalized_index.len()))
            .saturating_add(kind_working_bytes(&kind, participation));
        debug_assert_eq!(planned_working_bytes, actual_working_bytes);
        let source_order = scene.source_owner_order("elements", index);
        push_pending_event(
            &mut pending_events,
            source_order,
            PendingNormalizationEventKind::Record {
                event: normalization_event,
                preflighted: true,
                element_id: Some(memory_at_source!(try_owned(&id))),
                local_owner: Some(memory_at_source!(try_owned(&id))),
                json_pointer: memory_at_source!(try_owned(&element_pointer)),
                working_bytes: planned_working_bytes,
                work_units: normalization_work_units,
            },
        )?;
        memory_at_source!(used_ids.try_reserve(1));
        used_ids.push(memory_at_source!(try_owned(&id)));

        element_diagnostics.sort_by(|(left_position, left), (right_position, right)| {
            left_position
                .cmp(right_position)
                .then_with(|| left.json_pointer.cmp(&right.json_pointer))
                .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        });
        let repair_work_units = u64::try_from(element_diagnostics.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(8);
        for (position, diagnostic) in element_diagnostics {
            push_pending_event(
                &mut pending_events,
                source_order,
                PendingNormalizationEventKind::Diagnostic {
                    diagnostic,
                    local_owner: Some(memory_at_source!(try_owned(&id))),
                    source_position: position.as_u64(),
                },
            )?;
        }

        elements.push(NormalizedElement {
            source: NormalizedElementSource {
                source_index: index,
                id,
                pointer: element_pointer,
                coordinate_positions: [
                    source.member_position("x").unwrap_or(usize::MAX),
                    source.member_position("y").unwrap_or(usize::MAX),
                    source.member_position("width").unwrap_or(usize::MAX),
                    source.member_position("height").unwrap_or(usize::MAX),
                ],
            },
            source_order,
            index: normalized_index,
            x,
            y,
            width,
            height,
            angle: fields.angle,
            stroke_color: memory_at_source!(try_owned(fields.stroke_color)),
            background_color: memory_at_source!(try_owned(fields.background_color)),
            fill_style: memory_at_source!(try_owned(fields.fill_style)),
            stroke_width: fields.stroke_width,
            stroke_style: memory_at_source!(try_owned(fields.stroke_style)),
            roughness: fields.roughness,
            opacity: fields.opacity,
            link: match source.link {
                SourceField::Value(Some(link)) => {
                    memory_at_source!(crate::scene_surface::sanitized_link(link))
                }
                SourceField::Missing | SourceField::Invalid | SourceField::Value(None) => None,
            },
            frame_id: match source.frame_id {
                SourceField::Value(Some(frame_id)) if !frame_id.is_empty() => {
                    Some(memory_at_source!(try_owned(frame_id)))
                }
                SourceField::Missing
                | SourceField::Invalid
                | SourceField::Value(None | Some(_)) => None,
            },
            seed,
            roundness: normalized_roundness(&source, policy),
            normalized_working_bytes: planned_working_bytes,
            normalization_work_units,
            repair_work_units,
            kind_name: source.kind.scene_surface_kind(),
            kind,
            direct_outline_compatible: pending.direct_outline_compatible,
        });
    }

    let (frame_rendering, frame_diagnostics) = normalize_frame_rendering(
        scene.frame_rendering(),
        scene.app_state_member_position("frameRendering"),
        policy,
        format,
        surface_diagnostic_limit.saturating_sub(planned_diagnostic_count),
    )?;
    planned_diagnostic_count = planned_diagnostic_count.saturating_add(frame_diagnostics.len());
    let frame_source_order = scene.source_owner_order("appState", 0);
    push_pending_event(
        &mut pending_events,
        frame_source_order,
        PendingNormalizationEventKind::Record {
            event: "normalized_frame_rendering",
            preflighted: false,
            element_id: None,
            local_owner: None,
            json_pointer: try_owned("/appState/frameRendering")?,
            working_bytes: 4 * 64 + view_background_working_bytes,
            work_units: 5 * 8,
        },
    )?;
    for (position, diagnostic) in frame_diagnostics {
        push_pending_event(
            &mut pending_events,
            frame_source_order,
            PendingNormalizationEventKind::Diagnostic {
                diagnostic,
                local_owner: None,
                source_position: position,
            },
        )?;
    }
    let (files, file_diagnostics) = normalize_referenced_file_records(
        scene,
        &referenced_file_ids,
        format,
        surface_diagnostic_limit.saturating_sub(planned_diagnostic_count),
    )?;
    for file in &files {
        push_pending_event(
            &mut pending_events,
            file.source_order,
            PendingNormalizationEventKind::Record {
                event: "normalized_file_record",
                preflighted: false,
                element_id: None,
                local_owner: Some(try_owned(&file.key)?),
                json_pointer: try_owned(&file.source_pointer)?,
                working_bytes: file.normalized_working_bytes,
                work_units: file.normalization_work_units,
            },
        )?;
    }
    for (local_owner, source_order, source_position, diagnostic) in file_diagnostics {
        push_pending_event(
            &mut pending_events,
            source_order,
            PendingNormalizationEventKind::Diagnostic {
                diagnostic,
                local_owner: Some(local_owner),
                source_position,
            },
        )?;
    }

    replay_normalization_events(&mut pending_events, &mut observe)?;

    Ok(Some(NormalizationOutcome {
        elements,
        fallbacks: surface.fallbacks,
        view_background_color: try_owned(view_background_color)?,
        view_background_working_bytes,
        frame_rendering,
        files,
    }))
}

fn push_pending_event(
    events: &mut Vec<PendingNormalizationEvent>,
    source_order: u64,
    kind: PendingNormalizationEventKind,
) -> Result<(), TryReserveError> {
    let ordinal = u64::try_from(events.len()).unwrap_or(u64::MAX);
    events.try_reserve(1)?;
    events.push(PendingNormalizationEvent {
        source_order,
        ordinal,
        kind,
    });
    Ok(())
}

fn replay_normalization_events<E>(
    events: &mut Vec<PendingNormalizationEvent>,
    observe: &mut impl FnMut(NormalizationEvent<'_>) -> Result<(), E>,
) -> Result<(), NormalizationError<E>> {
    events.sort_unstable_by(|left, right| {
        left.source_order
            .cmp(&right.source_order)
            .then_with(|| match (&left.kind, &right.kind) {
                (
                    PendingNormalizationEventKind::Record { .. },
                    PendingNormalizationEventKind::Diagnostic { .. },
                ) => std::cmp::Ordering::Less,
                (
                    PendingNormalizationEventKind::Diagnostic { .. },
                    PendingNormalizationEventKind::Record { .. },
                ) => std::cmp::Ordering::Greater,
                (
                    PendingNormalizationEventKind::Diagnostic {
                        diagnostic: left_diagnostic,
                        source_position: left_position,
                        ..
                    },
                    PendingNormalizationEventKind::Diagnostic {
                        diagnostic: right_diagnostic,
                        source_position: right_position,
                        ..
                    },
                ) => left_position
                    .cmp(right_position)
                    .then_with(|| {
                        left_diagnostic
                            .json_pointer
                            .cmp(&right_diagnostic.json_pointer)
                    })
                    .then_with(|| {
                        left_diagnostic
                            .code
                            .as_str()
                            .cmp(right_diagnostic.code.as_str())
                    })
                    .then_with(|| left.ordinal.cmp(&right.ordinal)),
                (
                    PendingNormalizationEventKind::Record { .. },
                    PendingNormalizationEventKind::Record { .. },
                ) => left.ordinal.cmp(&right.ordinal),
            })
    });
    for event in events.drain(..) {
        match event.kind {
            PendingNormalizationEventKind::Record {
                event,
                preflighted,
                element_id,
                local_owner,
                json_pointer,
                working_bytes,
                work_units,
            } => {
                if !preflighted {
                    observe(NormalizationEvent::PreflightRecord {
                        event,
                        json_pointer: &json_pointer,
                        working_bytes,
                        work_units,
                    })
                    .map_err(NormalizationError::Observer)?;
                }
                observe(NormalizationEvent::CommitRecord {
                    event,
                    element_id: element_id.as_deref(),
                    local_owner: local_owner.as_deref(),
                    json_pointer: &json_pointer,
                    working_bytes,
                    work_units,
                })
                .map_err(NormalizationError::Observer)?;
            }
            PendingNormalizationEventKind::Diagnostic {
                diagnostic,
                local_owner,
                source_position,
            } => observe(NormalizationEvent::Diagnostic {
                diagnostic,
                local_owner: local_owner.as_deref(),
                source_order: event.source_order,
                source_position,
            })
            .map_err(NormalizationError::Observer)?,
        }
    }
    Ok(())
}

fn manifested_value<T: Copy>(
    source: SourceField<T>,
    default: T,
    accepts: impl FnOnce(T) -> bool,
) -> T {
    match source {
        SourceField::Value(value) if accepts(value) => value,
        SourceField::Missing | SourceField::Invalid | SourceField::Value(_) => default,
    }
}

fn source_accepts<T>(source: SourceField<T>, accepts: impl FnOnce(T) -> bool) -> bool {
    match source {
        SourceField::Missing => true,
        SourceField::Value(value) => accepts(value),
        SourceField::Invalid => false,
    }
}

type OwnedNormalizationDiagnostic = (String, u64, u64, CompatibilityDiagnostic);
type NormalizedFileRecords = (Vec<NormalizedFileRecord>, Vec<OwnedNormalizationDiagnostic>);

fn normalize_referenced_file_records(
    scene: &SceneDocument,
    referenced_file_ids: &[&str],
    format: OutputFormat,
    diagnostic_limit: usize,
) -> Result<NormalizedFileRecords, TryReserveError> {
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    files.try_reserve_exact(scene.file_record_count().min(referenced_file_ids.len()))?;
    for source_index in 0..scene.file_record_count() {
        let Some(source) = scene.file_record_at(source_index) else {
            continue;
        };
        if referenced_file_ids.binary_search(&source.key()).is_err()
            || !source.is_record()
            || source.key().is_empty()
        {
            continue;
        }
        let source_pointer = pointer_child("/files", source.key())?;
        let id = match source.id() {
            SourceField::Value(id) if !id.is_empty() => try_owned(id)?,
            SourceField::Missing | SourceField::Invalid | SourceField::Value(_) => {
                if diagnostics.len() < diagnostic_limit {
                    let pointer = pointer_child(&source_pointer, "id")?;
                    diagnostics.try_reserve(1)?;
                    diagnostics.push((
                        try_owned(source.key())?,
                        scene.source_owner_order("files", source_index),
                        u64::try_from(source.member_position("id").unwrap_or(0))
                            .unwrap_or(u64::MAX),
                        compatibility_diagnostic(
                            DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                            format,
                            &[DiagnosticEffect::InputRepair],
                            None,
                            &pointer,
                            "missing or invalid file identity was repaired to its map key",
                            DiagnosticRecovery::new(Recovery::Repaired, None),
                        )?,
                    ));
                }
                try_owned(source.key())?
            }
        };
        let created = match source.created() {
            SourceField::Value(created) => Some(created),
            SourceField::Invalid => None,
            SourceField::Missing => {
                if diagnostics.len() < diagnostic_limit {
                    let pointer = pointer_child(&source_pointer, "created")?;
                    diagnostics.try_reserve(1)?;
                    diagnostics.push((
                        try_owned(source.key())?,
                        scene.source_owner_order("files", source_index),
                        u64::try_from(source.member_position("created").unwrap_or(usize::MAX))
                            .unwrap_or(u64::MAX),
                        compatibility_diagnostic(
                            DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                            format,
                            &[DiagnosticEffect::InputRepair],
                            None,
                            &pointer,
                            "missing required file timestamp was repaired to canonical zero",
                            DiagnosticRecovery::new(Recovery::Repaired, None),
                        )?,
                    ));
                }
                Some(0)
            }
        };
        let normalized_working_bytes = 256_u64
            .saturating_add(owned_field_bytes(source.key().len()))
            .saturating_add(owned_field_bytes(id.len()))
            .saturating_add(u64::from(created.is_some()) * 64);
        let normalization_work_units = (1 + u64::from(created.is_some())) * 8;
        files.push(NormalizedFileRecord {
            key: try_owned(source.key())?,
            id,
            created,
            source_pointer,
            source_order: scene.source_owner_order("files", source_index),
            normalized_working_bytes,
            normalization_work_units,
        });
    }
    Ok((files, diagnostics))
}

fn normalize_frame_rendering(
    source: SourceField<FrameRenderingSource<'_>>,
    app_state_position: Option<usize>,
    policy: &crate::scene_surface::SceneSurfacePolicy,
    format: OutputFormat,
    diagnostic_limit: usize,
) -> Result<
    (
        NormalizedFrameRendering,
        Vec<(u64, CompatibilityDiagnostic)>,
    ),
    TryReserveError,
> {
    let mut diagnostics = Vec::new();
    let SourceField::Value(source) = source else {
        return Ok((
            NormalizedFrameRendering {
                enabled: true,
                clip: true,
                name: true,
                outline: true,
            },
            diagnostics,
        ));
    };
    let mut value = |key: &str| -> Result<bool, TryReserveError> {
        match source.boolean(key) {
            SourceField::Missing => Ok(true),
            SourceField::Value(value) => Ok(value),
            SourceField::Invalid => {
                if diagnostics.len() >= diagnostic_limit {
                    return Ok(true);
                }
                let pointer = pointer_child("/appState/frameRendering", key)?;
                diagnostics.try_reserve(1)?;
                diagnostics.push((
                    SourceDiagnosticPosition::nested(
                        app_state_position.unwrap_or(usize::MAX),
                        source.member_position(key).unwrap_or(usize::MAX),
                    )
                    .as_u64(),
                    compatibility_diagnostic(
                        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                        format,
                        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                        None,
                        &pointer,
                        "unsupported frame rendering control used the revision-1 default",
                        DiagnosticRecovery::new(Recovery::Substituted, None),
                    )?,
                ));
                Ok(true)
            }
        }
    };
    let normalized = NormalizedFrameRendering {
        enabled: value("enabled")?,
        clip: value("clip")?,
        name: value("name")?,
        outline: value("outline")?,
    };
    for (position, member) in source.members().iter().enumerate() {
        if diagnostics.len() >= diagnostic_limit {
            break;
        }
        let semantic_path = pointer_child("", &member.key)?;
        if crate::scene_surface::is_manifested_path(policy, "frame_rendering", &semantic_path) {
            continue;
        }
        diagnostics.try_reserve(1)?;
        diagnostics.push((
            SourceDiagnosticPosition::nested(
                app_state_position.unwrap_or(usize::MAX),
                position,
            )
            .as_u64(),
            compatibility_diagnostic(
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY,
                format,
                &[
                    DiagnosticEffect::Visual,
                    DiagnosticEffect::Semantic,
                    DiagnosticEffect::Metadata,
                ],
                None,
                &pointer_child("/appState/frameRendering", &member.key)?,
                "unmatched frame-rendering property was ignored by the revision-1 scene-surface policy",
                DiagnosticRecovery::new(Recovery::Ignored, None),
            )?,
        ));
    }
    diagnostics.sort_by(|(left_position, left), (right_position, right)| {
        left_position
            .cmp(right_position)
            .then_with(|| left.json_pointer.cmp(&right.json_pointer))
    });
    Ok((normalized, diagnostics))
}

fn generated_element_id(
    revision: CompatibilityRevision,
    source_index: u64,
    canonical_element: &[u8],
) -> Result<String, TryReserveError> {
    let mut hasher = Sha256::new();
    hasher.update(MISSING_ID_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(source_index.to_le_bytes());
    hasher.update(
        u64::try_from(canonical_element.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(canonical_element);
    generated_id_from_digest(&hasher.finalize())
}

fn generated_rough_seed(revision: CompatibilityRevision, normalized_id: &str) -> i32 {
    let mut hasher = Sha256::new();
    hasher.update(ROUGH_SEED_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(
        u64::try_from(normalized_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(normalized_id.as_bytes());
    let digest = hasher.finalize();
    let seed = i32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]) & i32::MAX;
    if seed == 0 {
        1
    } else {
        seed
    }
}

fn generated_duplicate_element_id(
    revision: CompatibilityRevision,
    source_index: u64,
    original_id: &str,
    canonical_element: &[u8],
) -> Result<String, TryReserveError> {
    let mut hasher = Sha256::new();
    hasher.update(DUPLICATE_ID_DOMAIN);
    hasher.update(revision.get().to_le_bytes());
    hasher.update(source_index.to_le_bytes());
    hasher.update(
        u64::try_from(original_id.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(original_id.as_bytes());
    hasher.update(
        u64::try_from(canonical_element.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(canonical_element);
    generated_id_from_digest(&hasher.finalize())
}

fn generated_id_from_digest(digest: &[u8]) -> Result<String, TryReserveError> {
    let mut id = String::new();
    id.try_reserve_exact("generated-".len() + digest.len() * 2)?;
    id.push_str("generated-");
    push_lower_hex(&mut id, digest);
    Ok(id)
}

fn unique_generated_id(
    mut candidate: String,
    revision: CompatibilityRevision,
    source_index: u64,
    used_ids: &[String],
) -> Result<String, TryReserveError> {
    let base = try_owned(&candidate)?;
    let mut collision_ordinal = 0_u64;
    while used_ids.iter().any(|used| used == &candidate) {
        collision_ordinal = collision_ordinal.saturating_add(1);
        let mut hasher = Sha256::new();
        hasher.update(ID_COLLISION_DOMAIN);
        hasher.update(revision.get().to_le_bytes());
        hasher.update(source_index.to_le_bytes());
        hasher.update(collision_ordinal.to_le_bytes());
        hasher.update(u64::try_from(base.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(base.as_bytes());
        candidate = generated_id_from_digest(&hasher.finalize())?;
    }
    Ok(candidate)
}

fn defaulted<T: Copy>(field: SourceField<T>, default: T) -> Option<T> {
    match field {
        SourceField::Missing => Some(default),
        SourceField::Value(value) => Some(value),
        SourceField::Invalid => None,
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

#[derive(Clone, Copy)]
struct DiagnosticRecovery<'a> {
    recovery: Recovery,
    recipe_id: Option<&'a str>,
}

impl<'a> DiagnosticRecovery<'a> {
    const fn new(recovery: Recovery, recipe_id: Option<&'a str>) -> Self {
        Self {
            recovery,
            recipe_id,
        }
    }
}

fn repair_diagnostic(
    code: DiagnosticCode,
    format: OutputFormat,
    effects: &[DiagnosticEffect],
    element_id: &str,
    json_pointer: &str,
    message: &str,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    compatibility_diagnostic(
        code,
        format,
        effects,
        Some(element_id),
        json_pointer,
        message,
        DiagnosticRecovery::new(Recovery::Repaired, None),
    )
}

fn compatibility_diagnostic(
    code: DiagnosticCode,
    format: OutputFormat,
    effects: &[DiagnosticEffect],
    element_id: Option<&str>,
    json_pointer: &str,
    message: &str,
    recovery: DiagnosticRecovery<'_>,
) -> Result<CompatibilityDiagnostic, TryReserveError> {
    let mut owned_effects = Vec::new();
    owned_effects.try_reserve_exact(effects.len())?;
    owned_effects.extend_from_slice(effects);
    Ok(CompatibilityDiagnostic {
        code,
        severity: DiagnosticSeverity::Warning,
        format,
        effects: owned_effects,
        element_id: element_id.map(try_owned).transpose()?,
        json_pointer: try_owned(json_pointer)?,
        message: try_owned(message)?,
        details: Some(Box::new(ErrorDetails::Compatibility {
            recovery: recovery.recovery,
            recipe_id: recovery.recipe_id.map(try_owned).transpose()?,
        })),
        order: DiagnosticOrder::default(),
    })
}

fn manifested_effects(effects: &[&str]) -> Result<Vec<DiagnosticEffect>, TryReserveError> {
    let mut manifested = Vec::new();
    manifested.try_reserve_exact(effects.len())?;
    for effect in effects {
        let effect = match *effect {
            "input_repair" => DiagnosticEffect::InputRepair,
            "visual" => DiagnosticEffect::Visual,
            "semantic" => DiagnosticEffect::Semantic,
            "metadata" => DiagnosticEffect::Metadata,
            _ => continue,
        };
        manifested.push(effect);
    }
    Ok(manifested)
}

const fn manifested_recovery(recovery: &str) -> Option<Recovery> {
    match recovery.as_bytes() {
        b"repaired" => Some(Recovery::Repaired),
        b"ignored" => Some(Recovery::Ignored),
        b"placeholder" => Some(Recovery::Placeholder),
        b"omitted" => Some(Recovery::Omitted),
        b"removed" => Some(Recovery::Removed),
        b"substituted" => Some(Recovery::Substituted),
        b"flattened" => Some(Recovery::Flattened),
        b"rasterized" => Some(Recovery::Rasterized),
        b"metadata_omitted" => Some(Recovery::MetadataOmitted),
        _ => None,
    }
}

fn try_owned(value: &str) -> Result<String, TryReserveError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len())?;
    owned.push_str(value);
    Ok(owned)
}

fn push_lower_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        codes_and_pointers, normalize_scene, pointers, reported_diagnostics, NormalizationOutcome,
        NormalizedElementKind, NormalizedRoundness,
    };
    use crate::{
        CompatibilityDiagnostic, CompatibilityRevision, DiagnosticCode, DiagnosticEffect,
        OutputFormat, SceneDocument, SupplementalAssets,
    };

    /// The diagnostics a default SVG render of `source` reports from
    /// normalization, in report order, with the normalized outcome.
    fn reported(source: &[u8]) -> (Vec<CompatibilityDiagnostic>, NormalizationOutcome) {
        reported_for(source, OutputFormat::Svg, true)
    }

    /// [`reported`] for `format`, with the request's frame-name control set
    /// to `frame_names`.
    fn reported_for(
        source: &[u8],
        format: OutputFormat,
        frame_names: bool,
    ) -> (Vec<CompatibilityDiagnostic>, NormalizationOutcome) {
        let scene = SceneDocument::parse(source).expect("normalization vector is admitted");
        reported_diagnostics(&scene, format, frame_names, &SupplementalAssets::default())
    }

    #[test]
    fn revision_1_identity_timestamp_and_seed_repairs_are_source_located() {
        let (diagnostics, normalized) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":180,"height":100,"seed":0}],"appState":{},"files":{}}"##,
        );
        assert_eq!(
            codes_and_pointers(&diagnostics),
            [
                (
                    DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID,
                    "/elements/0/id"
                ),
                (
                    DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                    "/elements/0/seed"
                ),
                (
                    DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                    "/elements/0/updated"
                ),
            ]
        );
        assert_eq!(diagnostics[0].effects(), &[DiagnosticEffect::InputRepair]);
        assert_eq!(
            diagnostics[1].effects(),
            &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual]
        );
        assert_eq!(diagnostics[2].effects(), &[DiagnosticEffect::InputRepair]);
        // The missing identity repair exposes the normalized identity, and
        // every repair on the element is attributed to it.
        let generated_id =
            "generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792";
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.element_id() == Some(generated_id)));
        for diagnostic in &diagnostics {
            let value = serde_json::to_value(diagnostic).expect("diagnostic serializes");
            assert_eq!(
                value["details"],
                serde_json::json!({
                    "kind": "compatibility",
                    "recovery": "repaired",
                    "recipe_id": null,
                })
            );
        }
        // The repaired values are the ones an already-normalized scene would
        // carry: the generated identity and the pinned seed for it. A scene
        // that spells them out normalizes to the same element and repairs
        // nothing.
        assert_eq!(normalized.elements[0].source.id(), generated_id);
        assert_eq!(normalized.elements[0].seed, 642_336_616);
        let (diagnostics, already_normalized) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"generated-32182208bba6af9fdf6995f59ee6a4097851c4655cf10d9e0681746fc495b792","type":"rectangle","width":180,"height":100,"seed":642336616,"updated":0}],"appState":{},"files":{}}"##,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(already_normalized.elements[0].source.id(), generated_id);
        assert_eq!(
            already_normalized.elements[0].seed,
            normalized.elements[0].seed
        );
        assert_eq!(
            already_normalized.elements[0].kind,
            normalized.elements[0].kind
        );
    }

    #[test]
    fn repair_diagnostics_carry_the_format_they_were_produced_for() {
        let source = br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","width":30,"height":20,"seed":0}],"appState":{},"files":{}}"##;
        for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
            let (diagnostics, _) = reported_for(source, format, true);
            assert_eq!(diagnostics.len(), 3, "{format:?}");
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.format() == format),
                "{format:?}"
            );
        }
    }

    #[test]
    fn revision_1_rekeys_each_later_duplicate_identity_in_source_order() {
        let (diagnostics, normalized) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"duplicate","type":"rectangle","x":0,"y":0,"width":80,"height":60,"seed":1,"updated":1},{"id":"duplicate","type":"rectangle","x":100,"y":0,"width":80,"height":60,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        );
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("the later duplicate produces exactly one repair diagnostic: {diagnostics:?}");
        };
        assert_eq!(
            diagnostic.code(),
            DiagnosticCode::NORMALIZATION_DUPLICATE_ELEMENT_ID
        );
        assert_eq!(diagnostic.json_pointer(), "/elements/1/id");
        assert_eq!(
            diagnostic.element_id(),
            Some("generated-3180035c77cb940ec4c79c4729441c64bad5072d494bd981721e7e1b9c85c3a3")
        );
        assert_eq!(diagnostic.effects(), &[DiagnosticEffect::InputRepair]);
        // Both elements survive: the first keeps its identity.
        assert_eq!(
            normalized
                .elements
                .iter()
                .map(|element| element.source.id())
                .collect::<Vec<_>>(),
            [
                "duplicate",
                "generated-3180035c77cb940ec4c79c4729441c64bad5072d494bd981721e7e1b9c85c3a3",
            ]
        );
    }

    #[test]
    fn generated_identity_collision_is_rekeyed_without_displacing_the_first_owner() {
        // The first element already carries the identity the second would
        // generate, so the second is rekeyed once more and the first is left
        // alone: one diagnostic, on the element that had no identity.
        let (diagnostics, normalized) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"generated-d02dc903e43157ace063528d673d5657c6e6433aa467e643d1b7a2cb9296386a","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"type":"rectangle","x":100,"width":40,"height":40,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        );
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("the missing identity produces one repair diagnostic: {diagnostics:?}");
        };
        assert_eq!(
            diagnostic.code(),
            DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID
        );
        assert_eq!(diagnostic.json_pointer(), "/elements/1/id");
        assert_eq!(
            diagnostic.element_id(),
            Some("generated-604f36b7127cb8ae0e242c4cf18cbb0f437f585790ef41c6f0edaa07a7676e63")
        );
        assert_eq!(
            normalized.elements[0].source.id(),
            "generated-d02dc903e43157ace063528d673d5657c6e6433aa467e643d1b7a2cb9296386a"
        );
    }

    #[test]
    fn invalid_frame_rendering_controls_are_repaired_for_empty_scenes() {
        let (diagnostics, normalized) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[],"appState":{"frameRendering":{"enabled":false,"clip":"yes"}},"files":{}}"##,
        );
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("the invalid frame control produces one diagnostic: {diagnostics:?}");
        };
        assert_eq!(
            diagnostic.code(),
            DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE
        );
        assert_eq!(diagnostic.json_pointer(), "/appState/frameRendering/clip");
        assert_eq!(
            diagnostic.effects(),
            &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual]
        );
        assert!(!normalized.frame_rendering.enabled);
        assert!(
            normalized.frame_rendering.clip,
            "the invalid clip is defaulted"
        );
    }

    #[test]
    fn assigned_element_repairs_are_reported_before_later_rendering_slices() {
        let (diagnostics, _) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","width":10,"height":10,"points":"invalid","seed":1,"updated":1},{"id":"image","type":"image","width":10,"height":10,"status":"future","seed":1,"updated":1}],"appState":{},"files":{}}"##,
        );
        assert_eq!(
            pointers(&diagnostics),
            [
                "/elements/0/points",
                "/elements/1/status",
                "/elements/1/fileId",
            ]
        );
    }

    #[test]
    fn frame_name_diagnostics_follow_the_requested_frame_name_control() {
        // With frame names disabled by the request, a malformed `name` is
        // never read and so never diagnosed, whatever the scene's own
        // `frameRendering.name` says.
        let name_only = br##"{"type":"excalidraw","version":2,"elements":[{"id":"frame","type":"frame","width":10,"height":10,"name":3,"seed":1,"updated":1}],"appState":{},"files":{}}"##;
        let (diagnostics, _) = reported_for(name_only, OutputFormat::Svg, false);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        // The request's frame-name control overrides a scene that turned
        // frame names off: the name is read and its value diagnosed.
        let scene_disabled = br##"{"type":"excalidraw","version":2,"elements":[{"id":"frame","type":"frame","width":10,"height":10,"name":3,"seed":1,"updated":1}],"appState":{"frameRendering":{"name":false}},"files":{}}"##;
        let (diagnostics, _) = reported_for(scene_disabled, OutputFormat::Svg, true);
        assert_eq!(
            codes_and_pointers(&diagnostics),
            [(
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                "/elements/0/name"
            )]
        );
    }

    #[test]
    fn normalization_reports_follow_preserved_root_owner_order() {
        // `files` precedes `elements` in this document, so the file record's
        // repairs come first although the element references the file.
        let (diagnostics, _) = reported(
            br##"{"type":"excalidraw","version":2,"files":{"asset":{}},"elements":[{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","status":"future","seed":1,"updated":1}],"appState":{}}"##,
        );
        assert_eq!(
            pointers(&diagnostics),
            [
                "/files/asset/id",
                "/files/asset/dataURL",
                "/files/asset/created",
                "/elements/0/status",
            ]
        );
    }

    #[test]
    fn normalization_diagnostics_follow_total_source_and_field_order() {
        let (diagnostics, _) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[{"type":"rectangle","x":0,"width":40,"height":40,"seed":0},{"id":"same","type":"rectangle","x":50,"width":40,"height":40,"seed":0},{"id":"same","type":"rectangle","x":100,"width":40,"height":40,"seed":0}],"appState":{},"files":{}}"##,
        );
        assert_eq!(
            codes_and_pointers(&diagnostics),
            [
                (
                    DiagnosticCode::NORMALIZATION_MISSING_ELEMENT_ID,
                    "/elements/0/id",
                ),
                (
                    DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                    "/elements/0/seed",
                ),
                (
                    DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                    "/elements/0/updated",
                ),
                (
                    DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                    "/elements/1/seed",
                ),
                (
                    DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                    "/elements/1/updated",
                ),
                (
                    DiagnosticCode::NORMALIZATION_DUPLICATE_ELEMENT_ID,
                    "/elements/2/id",
                ),
                (
                    DiagnosticCode::NORMALIZATION_ZERO_ROUGH_SEED,
                    "/elements/2/seed",
                ),
                (
                    DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                    "/elements/2/updated",
                ),
            ]
        );
    }

    #[test]
    fn normalization_order_respects_source_member_position() {
        // `seed` is spelled before `id` in the second element, so its repair
        // is reported first although identity is repaired first.
        let (diagnostics, _) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"same","type":"rectangle","x":0,"width":40,"height":40,"seed":1,"updated":1},{"seed":0,"id":"same","type":"rectangle","x":50,"width":40,"height":40,"updated":1}],"appState":{},"files":{}}"##,
        );
        assert_eq!(
            pointers(&diagnostics),
            ["/elements/1/seed", "/elements/1/id"]
        );
    }

    #[test]
    fn app_state_diagnostics_follow_source_member_order_across_surface_and_normalization() {
        // The unmatched members come from the scene-surface classification and
        // the frame-rendering control from compatibility normalization; spec
        // §11.3 orders them by the `appState` member they belong to regardless
        // of which layer reported them. Found by the scene fuzz.
        let (diagnostics, _) = reported(
            br##"{"type":"excalidraw","version":2,"elements":[],"appState":{"frameRendering":{"clip":"yes"},"alpha":1,"beta":2},"files":{}}"##,
        );
        assert_eq!(
            pointers(&diagnostics),
            [
                "/appState/frameRendering/clip",
                "/appState/alpha",
                "/appState/beta",
            ]
        );
    }

    #[test]
    fn linear_points_move_the_first_point_to_the_local_origin() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","x":30,"y":40,"width":20,"height":20,"points":[[3,4],[5,6]],"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("linear normalization vector is admitted");

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |_| Ok::<_, ()>(()),
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let [line] = normalized.elements.as_slice() else {
            panic!("one linear element is normalized");
        };
        let NormalizedElementKind::Linear(line_kind) = &line.kind else {
            panic!("the line retains its normalized linear kind");
        };

        assert_eq!((line.x, line.y), (33.0, 44.0));
        assert_eq!((line.width, line.height), (2.0, 2.0));
        assert_eq!(line_kind.points, [[0.0, 0.0], [2.0, 2.0]]);
    }

    #[test]
    fn linear_point_restore_precedes_common_negative_dimension_restore() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","x":30,"y":40,"width":-20,"height":-10,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("negative linear normalization vector is admitted");

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |_| Ok::<_, ()>(()),
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let line = &normalized.elements[0];
        let NormalizedElementKind::Linear(line_kind) = &line.kind else {
            panic!("the line retains normalized linear state");
        };

        assert_eq!((line.x, line.y), (30.0, 40.0));
        assert_eq!((line.width, line.height), (20.0, 10.0));
        assert_eq!(line_kind.points, [[0.0, 0.0], [-20.0, -10.0]]);
    }

    #[test]
    fn short_linear_point_arrays_use_the_silent_pinned_migration() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","width":20,"height":10,"points":[[1,1]],"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("short linear-point vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Linear(line) = &normalized.elements[0].kind else {
            panic!("the line retains normalized linear state");
        };

        assert_eq!(line.points, [[0.0, 0.0], [20.0, 10.0]]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ordered_nonempty_legacy_indexes_remain_anchored() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"first","index":"foo","type":"rectangle","width":10,"height":10,"seed":1,"updated":1},{"id":"second","index":"goo","type":"rectangle","width":10,"height":10,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("legacy index vector is admitted");

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |_| Ok::<_, ()>(()),
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");

        assert_eq!(normalized.elements[0].index, "foo");
        assert_eq!(normalized.elements[1].index, "goo");
    }

    #[test]
    fn elbow_bindings_and_arrow_defaults_match_the_pinned_restore_path() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"arrow","type":"arrow","x":0,"y":0,"width":100,"height":80,"points":[[0,0],[100,0],[100,80]],"elbowed":true,"startBinding":{"elementId":"target","gap":1,"fixedPoint":[0.5,0.49991]},"endBinding":{"elementId":"target","focus":0,"gap":1},"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("elbow normalization vector is admitted");

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |_| Ok::<_, ()>(()),
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Linear(arrow) = &normalized.elements[0].kind else {
            panic!("the arrow retains its normalized linear kind");
        };

        assert_eq!(arrow.kind, super::NormalizedLinearKind::Arrow);
        assert_eq!(arrow.start_arrowhead, None);
        assert_eq!(arrow.end_arrowhead.as_deref(), Some("arrow"));
        assert_eq!(arrow.last_committed_point, None);
        assert_eq!(
            arrow.start_binding.as_ref().map(|binding| (
                binding.element_id.as_str(),
                binding.focus,
                binding.gap,
                binding.fixed_point,
            )),
            Some(("target", 0.0, 1.0, Some([0.5001, 0.5001])))
        );
        assert_eq!(arrow.end_binding, None);
    }

    #[test]
    fn image_frame_and_frame_rendering_state_is_normalized_without_mutating_source() {
        let source = br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":80,"height":60,"fileId":"asset","status":"saved","scale":[-1,1],"crop":{"x":1,"y":2,"width":30,"height":20,"naturalWidth":60,"naturalHeight":40},"seed":1,"updated":1},{"id":"frame","type":"frame","width":100,"height":100,"name":"Named","seed":1,"updated":1},{"id":"magic","type":"magicframe","width":100,"height":100,"seed":1,"updated":1}],"appState":{"frameRendering":{"enabled":false,"clip":true}},"files":{"asset":{"id":"asset","created":1}}}"##;
        let scene = SceneDocument::parse(source).expect("state normalization vector is admitted");
        let canonical_before = scene
            .canonicalized_original_bytes()
            .expect("the original scene serializes");

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |_| Ok::<_, ()>(()),
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");

        let NormalizedElementKind::Image(image) = &normalized.elements[0].kind else {
            panic!("the image retains normalized image state");
        };
        assert_eq!(image.file_id.as_deref(), Some("asset"));
        assert_eq!(image.status, super::NormalizedImageStatus::Saved);
        assert_eq!(image.scale, [-1, 1]);
        assert_eq!(
            image.crop,
            Some(super::NormalizedImageCrop {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 20.0,
                natural_width: 60.0,
                natural_height: 40.0,
            })
        );
        let NormalizedElementKind::Frame(frame) = &normalized.elements[1].kind else {
            panic!("the frame retains normalized frame state");
        };
        let NormalizedElementKind::MagicFrame(magic) = &normalized.elements[2].kind else {
            panic!("the magic frame retains normalized frame state");
        };
        assert_eq!(frame.name.as_deref(), Some("Named"));
        assert_eq!(magic.name, None);
        assert_eq!(
            normalized.frame_rendering,
            super::NormalizedFrameRendering {
                enabled: false,
                clip: true,
                name: true,
                outline: true,
            }
        );
        assert_eq!(
            scene
                .canonicalized_original_bytes()
                .expect("the original scene still serializes"),
            canonical_before
        );
    }

    #[test]
    fn freedraw_state_uses_closed_defaults_and_retains_valid_samples() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"free","type":"freedraw","width":10,"height":10,"points":[[0,0],[10,10]],"pressures":[0.25,0.75],"simulatePressure":false,"lastCommittedPoint":[10,10],"seed":1,"updated":1},{"id":"defaults","type":"freedraw","width":10,"height":10,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("freedraw normalization vector is admitted");

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |_| Ok::<_, ()>(()),
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Freedraw(explicit) = &normalized.elements[0].kind else {
            panic!("the explicit freedraw state is normalized");
        };
        let NormalizedElementKind::Freedraw(defaulted) = &normalized.elements[1].kind else {
            panic!("the defaulted freedraw state is normalized");
        };

        assert_eq!(explicit.points, [[0.0, 0.0], [10.0, 10.0]]);
        assert_eq!(explicit.pressures, [0.25, 0.75]);
        assert!(!explicit.simulate_pressure);
        assert_eq!(explicit.last_committed_point, None);
        assert!(defaulted.points.is_empty());
        assert!(defaulted.pressures.is_empty());
        assert!(defaulted.simulate_pressure);
    }

    #[test]
    fn malformed_assigned_fields_emit_source_located_substitution_diagnostics() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","width":10,"height":10,"points":"invalid","seed":1,"updated":1},{"id":"image","type":"image","width":10,"height":10,"status":"loading","scale":[0,1],"crop":"invalid","seed":1,"updated":1},{"id":"frame","type":"frame","width":10,"height":10,"name":3,"seed":1,"updated":1}],"appState":{"frameRendering":{"clip":"yes"}},"files":{}}"##,
        )
        .expect("malformed normalization vector is admitted");
        let mut diagnostics = Vec::new();

        normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");

        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer.as_str()))
                .collect::<Vec<_>>(),
            [
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/0/points",
                ),
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/1/status",
                ),
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/1/scale",
                ),
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/1/crop",
                ),
                (DiagnosticCode::IMAGE_MISSING_FILE, "/elements/1/fileId",),
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/2/name",
                ),
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/appState/frameRendering/clip",
                ),
            ]
        );
        assert!(diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code != DiagnosticCode::IMAGE_MISSING_FILE)
            .all(|diagnostic| {
                serde_json::to_value(diagnostic).expect("diagnostic serializes")["details"]
                    ["recovery"]
                    == "substituted"
            }));
        assert_eq!(
            serde_json::to_value(
                diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.code == DiagnosticCode::IMAGE_MISSING_FILE)
                    .expect("missing image diagnostic is retained"),
            )
            .expect("diagnostic serializes")["details"]["recovery"],
            "omitted"
        );
    }

    #[test]
    fn only_referenced_file_records_receive_deterministic_record_repairs() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","seed":1,"updated":1}],"appState":{},"files":{"asset":{"mimeType":"image/png","dataURL":"data:image/png;base64,AA=="},"unused":{}}}"##,
        )
        .expect("file-record normalization vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");

        assert_eq!(normalized.files.len(), 1);
        assert_eq!(normalized.files[0].key, "asset");
        assert_eq!(normalized.files[0].id, "asset");
        assert_eq!(normalized.files[0].created, Some(0));
        assert_eq!(normalized.files[0].source_pointer, "/files/asset");
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.code(), diagnostic.json_pointer.as_str()))
                .collect::<Vec<_>>(),
            [
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/files/asset/id",
                ),
                (
                    DiagnosticCode::NORMALIZATION_MISSING_TIMESTAMP,
                    "/files/asset/created",
                ),
            ]
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.element_id.is_none()));
    }

    #[test]
    fn invalid_file_created_values_remain_ignored() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","created":"invalid","dataURL":"data:image/png;base64,AA=="}}}"##,
        )
        .expect("invalid file timestamp vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");

        assert_eq!(normalized.files[0].created, None);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn malformed_elbow_binding_members_use_their_narrowest_recovery() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"arrow","type":"arrow","width":10,"height":10,"points":[[0,0],[10,10]],"elbowed":true,"startBinding":{"elementId":"target","gap":1,"fixedPoint":"invalid"},"endBinding":{"elementId":"target","fixedPoint":[0,0]},"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("binding recovery vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Linear(arrow) = &normalized.elements[0].kind else {
            panic!("the arrow retains normalized linear state");
        };

        assert_eq!(
            arrow
                .start_binding
                .as_ref()
                .and_then(|binding| binding.fixed_point),
            Some([0.0, 0.0])
        );
        assert_eq!(arrow.end_binding, None);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.json_pointer.as_str())
                .collect::<Vec<_>>(),
            [
                "/elements/0/startBinding/fixedPoint",
                "/elements/0/endBinding/gap",
            ]
        );
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| {
                    serde_json::to_value(diagnostic).expect("diagnostic serializes")["details"]
                        ["recovery"]
                        .as_str()
                        .expect("recovery is a string")
                        .to_owned()
                })
                .collect::<Vec<_>>(),
            ["substituted", "omitted"]
        );
    }

    #[test]
    fn a_missing_nested_binding_member_keeps_its_parent_members_source_order() {
        // Spec §11.3 orders diagnostics by the owning source member before the
        // nested position. A missing `gap` under `startBinding` therefore
        // precedes an invalid `elementId` under the later `endBinding`, even
        // though the missing member has no nested position of its own. Found
        // by the scene fuzz, whose saturated nested ordinal used to spill out
        // of the member lane and sort the diagnostic after the whole element.
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"arrow","type":"arrow","width":10,"height":10,"points":[[0,0],[10,10]],"startBinding":{"elementId":"target"},"endBinding":{"elementId":""},"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("binding order vector is admitted");
        let mut diagnostics = Vec::new();

        normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");

        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.json_pointer.as_str())
                .collect::<Vec<_>>(),
            [
                "/elements/0/startBinding/gap",
                "/elements/0/endBinding/elementId",
            ]
        );
    }

    #[test]
    fn malformed_nested_linear_point_marks_only_that_geometry_unavailable() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","width":10,"height":10,"points":[[0,0],[10,"invalid"]],"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("nested-point recovery vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Linear(line) = &normalized.elements[0].kind else {
            panic!("the line retains normalized linear state");
        };

        assert!(!line.geometry_available);
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("one malformed point coordinate is diagnosed");
        };
        assert_eq!(diagnostic.json_pointer, "/elements/0/points/1/1");
        assert_eq!(
            serde_json::to_value(diagnostic).expect("diagnostic serializes")["details"],
            serde_json::json!({
                "kind": "compatibility",
                "recovery": "placeholder",
                "recipe_id": "renderer.known-element-unavailable.v1",
            })
        );
    }

    #[test]
    fn invalid_crop_members_disable_crop_and_report_each_member() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","crop":{"x":-1},"seed":1,"updated":1}],"appState":{},"files":{"asset":{"id":"asset","created":1,"dataURL":"data:image/png;base64,AA=="}}}"##,
        )
        .expect("crop recovery vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Image(image) = &normalized.elements[0].kind else {
            panic!("the image retains normalized image state");
        };

        assert_eq!(image.crop, None);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.json_pointer.as_str())
                .collect::<Vec<_>>(),
            [
                "/elements/0/crop/x",
                "/elements/0/crop/y",
                "/elements/0/crop/width",
                "/elements/0/crop/height",
                "/elements/0/crop/naturalWidth",
                "/elements/0/crop/naturalHeight",
            ]
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.effects == [DiagnosticEffect::Visual]
                && serde_json::to_value(diagnostic).expect("diagnostic serializes")["details"]
                    ["recovery"]
                    == "omitted"
        }));
    }

    #[test]
    fn invalid_arrow_defaults_are_substituted_and_diagnosed() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"arrow","type":"arrow","width":10,"height":10,"points":[[0,0],[10,10]],"elbowed":"yes","startArrowhead":"future","endArrowhead":3,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("arrow default vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Linear(arrow) = &normalized.elements[0].kind else {
            panic!("the arrow retains normalized linear state");
        };

        assert!(!arrow.elbowed);
        assert_eq!(arrow.start_arrowhead, None);
        assert_eq!(arrow.end_arrowhead.as_deref(), Some("arrow"));
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.json_pointer.as_str())
                .collect::<Vec<_>>(),
            [
                "/elements/0/elbowed",
                "/elements/0/startArrowhead",
                "/elements/0/endArrowhead",
            ]
        );
    }

    #[test]
    fn invalid_line_arrowhead_metadata_is_substituted_and_diagnosed() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"line","type":"line","width":10,"height":10,"points":[[0,0],[10,10]],"startArrowhead":"future","endArrowhead":3,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("line arrowhead vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Linear(line) = &normalized.elements[0].kind else {
            panic!("the line retains normalized linear state");
        };

        assert_eq!(line.start_arrowhead, None);
        assert_eq!(line.end_arrowhead, None);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.json_pointer.as_str())
                .collect::<Vec<_>>(),
            ["/elements/0/startArrowhead", "/elements/0/endArrowhead",]
        );
    }

    #[test]
    fn invalid_freedraw_state_uses_the_declared_local_recoveries() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"free","type":"freedraw","width":10,"height":10,"points":"invalid","pressures":"invalid","simulatePressure":"no","seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("freedraw recovery vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Freedraw(freedraw) = &normalized.elements[0].kind else {
            panic!("the freedraw element retains normalized state");
        };

        assert!(!freedraw.geometry_available);
        assert!(freedraw.points.is_empty());
        assert!(freedraw.pressures.is_empty());
        assert!(freedraw.simulate_pressure);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.json_pointer.as_str())
                .collect::<Vec<_>>(),
            [
                "/elements/0/points",
                "/elements/0/pressures",
                "/elements/0/simulatePressure",
            ]
        );
    }

    #[test]
    fn malformed_freedraw_points_do_not_invalidate_valid_pressures() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"free","type":"freedraw","width":10,"height":10,"points":[[0,0],[10,"invalid"]],"pressures":[0,1],"simulatePressure":false,"seed":1,"updated":1}],"appState":{},"files":{}}"##,
        )
        .expect("freedraw sample recovery vector is admitted");
        let mut diagnostics = Vec::new();

        let normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push(diagnostic);
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        let NormalizedElementKind::Freedraw(freedraw) = &normalized.elements[0].kind else {
            panic!("the freedraw element retains normalized state");
        };

        assert!(!freedraw.geometry_available);
        assert_eq!(freedraw.pressures, [0.0, 1.0]);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.json_pointer.as_str())
                .collect::<Vec<_>>(),
            ["/elements/0/points/1/1"]
        );
    }

    #[test]
    fn normalization_resource_events_have_stable_record_names_and_owners() {
        let scene = SceneDocument::parse(
            br##"{"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":10,"height":10,"fileId":"asset","seed":1,"updated":1}],"appState":{},"files":{"asset":{}}}"##,
        )
        .expect("resource-event vector is admitted");
        let mut events = Vec::new();
        let mut diagnostic_owners = Vec::new();

        normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                match event {
                    super::NormalizationEvent::CommitRecord {
                        event,
                        element_id,
                        local_owner,
                        json_pointer,
                        work_units,
                        ..
                    } => events.push((
                        event,
                        element_id.map(str::to_owned),
                        local_owner.map(str::to_owned),
                        json_pointer.to_owned(),
                        work_units,
                    )),
                    super::NormalizationEvent::Diagnostic {
                        diagnostic,
                        local_owner,
                        ..
                    } => diagnostic_owners
                        .push((diagnostic.json_pointer, local_owner.map(str::to_owned))),
                    super::NormalizationEvent::PreflightRecord { .. } => {}
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");

        assert_eq!(
            events,
            [
                (
                    "normalized_image",
                    Some("image".to_owned()),
                    Some("image".to_owned()),
                    "/elements/0".to_owned(),
                    19 * 8,
                ),
                (
                    "normalized_frame_rendering",
                    None,
                    None,
                    "/appState/frameRendering".to_owned(),
                    5 * 8,
                ),
                (
                    "normalized_file_record",
                    None,
                    Some("asset".to_owned()),
                    "/files/asset".to_owned(),
                    2 * 8,
                ),
            ]
        );
        assert_eq!(
            diagnostic_owners,
            [
                ("/files/asset/id".to_owned(), Some("asset".to_owned()),),
                ("/files/asset/dataURL".to_owned(), Some("asset".to_owned()),),
                ("/files/asset/created".to_owned(), Some("asset".to_owned()),),
            ]
        );
    }

    /// Normalizes a one-element scene whose element is `element_json` and
    /// returns that element with the element-scoped diagnostics normalization
    /// emitted for it.
    fn normalized_single_element(
        element_json: &str,
    ) -> (super::NormalizedElement, Vec<(DiagnosticCode, String)>) {
        let scene = format!(
            r##"{{"type":"excalidraw","version":2,"elements":[{element_json}],"appState":{{}},"files":{{}}}}"##
        );
        let scene =
            SceneDocument::parse(scene.as_bytes()).expect("single-element vector is admitted");
        let mut diagnostics = Vec::new();
        let mut normalized = normalize_scene(
            &scene,
            CompatibilityRevision::REVISION_1,
            OutputFormat::Svg,
            |event| {
                if let super::NormalizationEvent::Diagnostic { diagnostic, .. } = event {
                    diagnostics.push((diagnostic.code(), diagnostic.json_pointer.clone()));
                }
                Ok::<_, ()>(())
            },
        )
        .expect("normalization succeeds")
        .expect("the scene is in the implemented normalization slice");
        assert_eq!(normalized.elements.len(), 1, "one element is normalized");
        (normalized.elements.remove(0), diagnostics)
    }

    /// Normalizes one text element and returns its line height with the
    /// element-scoped diagnostics normalization emitted for it.
    fn normalized_text_line_height(text_fields: &str) -> (f64, Vec<(DiagnosticCode, String)>) {
        let (element, diagnostics) = normalized_single_element(&format!(
            r##"{{"id":"text","type":"text","seed":1,"updated":1,{text_fields}}}"##
        ));
        let NormalizedElementKind::Text(text) = &element.kind else {
            panic!("the element retains its normalized text kind");
        };
        (text.line_height, diagnostics)
    }

    #[test]
    fn an_explicit_positive_line_height_is_kept_verbatim() {
        let (line_height, diagnostics) = normalized_text_line_height(
            r##""text":"one","fontSize":20,"fontFamily":2,"width":40,"height":90,"lineHeight":1.4"##,
        );
        assert_eq!(line_height, 1.4);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn a_missing_line_height_detects_from_the_height_line_count_and_font_size() {
        // Excalidraw's restore path: height / splitIntoLines(text).length / fontSize.
        let (line_height, diagnostics) = normalized_text_line_height(
            r##""text":"one","fontSize":20,"fontFamily":2,"width":40,"height":30"##,
        );
        assert_eq!(line_height, 1.5);
        assert!(diagnostics.is_empty());

        let (line_height, _) = normalized_text_line_height(
            r##""text":"one\ntwo\nthree","fontSize":10,"fontFamily":3,"width":40,"height":36"##,
        );
        assert_eq!(line_height, 1.2);
    }

    #[test]
    fn a_missing_line_height_detects_with_the_normalized_font_size() {
        // The reference divides by the raw fontSize and yields NaN for a missing
        // or invalid one; revision 1 detects with the defaulted 20 instead (#108).
        let (line_height, diagnostics) =
            normalized_text_line_height(r##""text":"one","fontFamily":5,"width":40,"height":30"##);
        assert_eq!(line_height, 1.5);
        assert!(diagnostics.is_empty());

        let (line_height, diagnostics) = normalized_text_line_height(
            r##""text":"one","fontSize":-8,"fontFamily":5,"width":40,"height":30"##,
        );
        assert_eq!(line_height, 1.5);
        assert_eq!(
            diagnostics,
            [(
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                "/elements/0/fontSize".to_owned(),
            )]
        );
    }

    #[test]
    fn a_missing_line_height_counts_lines_after_eol_normalization() {
        // splitIntoLines normalizes \r\n and \r to \n before splitting; tab
        // expansion does not add lines. Four lines here, so 120 / 4 / 20.
        let (line_height, _) = normalized_text_line_height(
            r##""text":"a\r\nb\rc\n\td","fontSize":20,"fontFamily":5,"width":40,"height":120"##,
        );
        assert_eq!(line_height, 1.5);

        // A trailing line break still starts an (empty) final line.
        let (line_height, _) = normalized_text_line_height(
            r##""text":"a\n","fontSize":20,"fontFamily":5,"width":40,"height":60"##,
        );
        assert_eq!(line_height, 1.5);
    }

    #[test]
    fn a_missing_line_height_without_a_height_takes_the_family_metadata_line_height() {
        // FONT_METADATA[family].metrics.lineHeight, keyed on the source family:
        // Helvetica keeps 1.15 although Liberation Sans shapes it.
        for (family, expected) in [
            (1, 1.25),
            (2, 1.15),
            (3, 1.2),
            (5, 1.25),
            (6, 1.35),
            (7, 1.15),
            (8, 1.25),
            (9, 1.15),
        ] {
            let (line_height, diagnostics) = normalized_text_line_height(&format!(
                r##""text":"one","fontSize":20,"fontFamily":{family},"width":40"##
            ));
            assert_eq!(line_height, expected, "family {family}");
            assert!(diagnostics.is_empty(), "family {family}");
        }

        // An unrecognized family normalizes to Excalifont first, which is
        // also the reference's fallback for an unregistered family.
        let (line_height, diagnostics) = normalized_text_line_height(
            r##""text":"one","fontSize":20,"fontFamily":4,"width":40"##,
        );
        assert_eq!(line_height, 1.25);
        assert_eq!(
            diagnostics,
            [(
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                "/elements/0/fontFamily".to_owned(),
            )]
        );
    }

    #[test]
    fn a_zero_height_takes_the_family_metadata_line_height_like_the_reference_truthiness() {
        let (line_height, diagnostics) = normalized_text_line_height(
            r##""text":"one","fontSize":20,"fontFamily":6,"width":40,"height":0"##,
        );
        assert_eq!(line_height, 1.35);
        assert!(diagnostics.is_empty());

        // Negative heights are normalized to their magnitude before detection.
        let (line_height, _) = normalized_text_line_height(
            r##""text":"one","fontSize":20,"fontFamily":6,"width":40,"height":-30"##,
        );
        assert_eq!(line_height, 1.5);
    }

    #[test]
    fn non_positive_line_heights_derive_and_keep_the_invalid_diagnostic() {
        // The reference's `||` treats 0 as missing and keeps a negative;
        // revision 1 derives both, and both keep the existing unsupported
        // diagnostic.
        for source in ["0", "-1.2"] {
            let (line_height, diagnostics) = normalized_text_line_height(&format!(
                r##""text":"one\ntwo","fontSize":20,"fontFamily":2,"width":40,"height":60,"lineHeight":{source}"##
            ));
            assert_eq!(line_height, 1.5, "lineHeight {source}");
            assert_eq!(
                diagnostics,
                [(
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/0/lineHeight".to_owned(),
                )],
                "lineHeight {source}"
            );

            let (line_height, _) = normalized_text_line_height(&format!(
                r##""text":"one","fontSize":20,"fontFamily":2,"width":40,"lineHeight":{source}"##
            ));
            assert_eq!(line_height, 1.15, "lineHeight {source} without height");
        }
    }

    /// Normalizes one text element and returns its font size and family with
    /// the element-scoped diagnostics normalization emitted for it.
    fn normalized_text_font(text_fields: &str) -> ((f64, i32), Vec<(DiagnosticCode, String)>) {
        let (element, diagnostics) = normalized_single_element(&format!(
            r##"{{"id":"text","type":"text","seed":1,"updated":1,"text":"one","width":40,"height":35,{text_fields}}}"##
        ));
        let NormalizedElementKind::Text(text) = &element.kind else {
            panic!("the element retains its normalized text kind");
        };
        ((text.font_size, text.font_family), diagnostics)
    }

    #[test]
    fn a_legacy_font_string_migrates_to_font_size_and_font_family() {
        // restoreElement, text case: `font.split(" ")` gives the pixel token
        // and the family name; parseFloat strips the unit and FONT_FAMILY maps
        // the name to its id. The migration is silent.
        let (font, diagnostics) = normalized_text_font(r##""font":"28px Virgil""##);
        assert_eq!(font, (28.0, 1));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let (font, diagnostics) = normalized_text_font(r##""font":"14.5px Cascadia""##);
        assert_eq!(font, (14.5, 3));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn a_legacy_font_family_name_maps_like_font_family_and_unknown_names_fall_back() {
        // getFontFamilyByName: `Object.keys(FONT_FAMILY).includes(name)` is an
        // exact, case-sensitive match; anything else is DEFAULT_FONT_FAMILY
        // (Excalifont, 5). The lookup takes the second space-separated token
        // only, so a multi-word name such as "Lilita One" never matches, and
        // a missing token (`undefined`) falls back the same way, silently.
        for (font, expected_family) in [
            ("20px Virgil", 1),
            ("20px Helvetica", 2),
            ("20px Cascadia", 3),
            ("20px Excalifont", 5),
            ("20px Nunito", 6),
            ("20px Lilita One", 5),
            ("20px Comic Shanns", 5),
            ("20px Liberation Sans", 5),
            ("20px virgil", 5),
            ("20px Arial", 5),
            ("20px", 5),
            ("20px  Virgil", 5),
        ] {
            let (font_values, diagnostics) = normalized_text_font(&format!(r##""font":"{font}""##));
            assert_eq!(font_values, (20.0, expected_family), "{font:?}");
            assert!(diagnostics.is_empty(), "{font:?}: {diagnostics:?}");
        }
    }

    #[test]
    fn a_legacy_font_size_reads_like_parse_float() {
        // parseFloat takes the longest decimal-literal prefix of the first
        // token: the unit is ignored whatever it is, a fraction may lack its
        // integer part, and an exponent is honored only when digits follow.
        for (font, expected_size) in [
            ("20 Virgil", 20.0),
            ("20em Virgil", 20.0),
            ("+20px Virgil", 20.0),
            (".5px Virgil", 0.5),
            ("20.px Virgil", 20.0),
            ("1e2px Virgil", 100.0),
            ("1.5E+1px Virgil", 15.0),
            ("2e Virgil", 2.0),
            ("2e+px Virgil", 2.0),
        ] {
            let (font_values, diagnostics) = normalized_text_font(&format!(r##""font":"{font}""##));
            assert_eq!(font_values, (expected_size, 1), "{font:?}");
            assert!(diagnostics.is_empty(), "{font:?}: {diagnostics:?}");
        }
    }

    #[test]
    fn a_legacy_font_string_wins_over_the_modern_fields() {
        // `if ("font" in element)` overwrites both fontSize and fontFamily
        // whenever the key is present, and the key survives a re-save, so a
        // legacy scene edited in a modern editor still renders at the `font`
        // values. A modern field outside its domain keeps its own diagnostic.
        let (font, diagnostics) =
            normalized_text_font(r##""font":"28px Virgil","fontSize":40,"fontFamily":2"##);
        assert_eq!(font, (28.0, 1));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let (font, diagnostics) =
            normalized_text_font(r##""font":"28px Virgil","fontSize":"big","fontFamily":4"##);
        assert_eq!(font, (28.0, 1));
        assert_eq!(
            diagnostics,
            [
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/0/fontSize".to_owned(),
                ),
                (
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/0/fontFamily".to_owned(),
                ),
            ]
        );
    }

    #[test]
    fn a_legacy_font_string_leaves_line_height_detection_to_the_source_fields() {
        // The restore computes lineHeight before the migration lands:
        // `detectLineHeight` divides by the element's own fontSize and
        // `getLineHeight` reads its own fontFamily, so the modern fields, not
        // the parsed ones, decide the restored line height (#108).
        let (line_height, diagnostics) = normalized_text_line_height(
            r##""text":"one","font":"20px Virgil","fontSize":40,"width":40,"height":50"##,
        );
        assert_eq!(line_height, 1.25);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        // Without a modern fontSize the reference detects NaN; revision 1
        // detects with the migrated size, as it does with the default (#108).
        let (line_height, diagnostics) = normalized_text_line_height(
            r##""text":"one","font":"36px Virgil","width":40,"height":45"##,
        );
        assert_eq!(line_height, 1.25);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        // Without a height the family metadata line height is the source
        // family's: Cascadia's 1.2 here although Helvetica shapes the text,
        // and Excalifont's 1.25 for the unregistered `undefined` when the
        // legacy string is the only family the element names.
        let (line_height, _) = normalized_text_line_height(
            r##""text":"one","font":"20px Helvetica","fontFamily":3,"width":40"##,
        );
        assert_eq!(line_height, 1.2);
        let (line_height, _) =
            normalized_text_line_height(r##""text":"one","font":"20px Helvetica","width":40"##);
        assert_eq!(line_height, 1.25);
    }

    #[test]
    fn a_malformed_legacy_font_takes_the_independent_field_recovery_and_is_diagnosed() {
        // A string whose size token parseFloat cannot read (NaN), or reads as
        // a size nothing can render, is the reference's broken output; the
        // manifest's `/font` recovery substitutes the independent fontSize
        // and fontFamily dispositions and reports `unsupported-property-value`
        // at `/font`. Present modern fields then apply as if `font` were absent.
        for font in [
            r##""""##,
            r##""px Virgil""##,
            r##""bold 20px Virgil""##,
            r##"" 20px Virgil""##,
            r##""0px Virgil""##,
            r##""-20px Virgil""##,
            r##""0x10px Virgil""##,
            r##""Infinity Virgil""##,
        ] {
            let (font_values, diagnostics) = normalized_text_font(&format!(r##""font":{font}"##));
            assert_eq!(font_values, (20.0, 5), "{font}");
            assert_eq!(
                diagnostics,
                [(
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/0/font".to_owned(),
                )],
                "{font}"
            );

            let (font_values, diagnostics) =
                normalized_text_font(&format!(r##""font":{font},"fontSize":32,"fontFamily":3"##));
            assert_eq!(font_values, (32.0, 3), "{font} with modern fields");
            assert_eq!(
                diagnostics,
                [(
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/0/font".to_owned(),
                )],
                "{font} with modern fields"
            );
        }

        // A non-string is outside the manifest's `string` domain and takes the
        // same recovery; the reference would throw on `.split`.
        for font in ["null", "20", r##"{"size":20}"##] {
            let (font_values, diagnostics) = normalized_text_font(&format!(r##""font":{font}"##));
            assert_eq!(font_values, (20.0, 5), "{font}");
            assert_eq!(
                diagnostics,
                [(
                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                    "/elements/0/font".to_owned(),
                )],
                "{font}"
            );
        }
    }

    /// Normalizes one element of `kind` and returns its roundness with the
    /// element-scoped diagnostics normalization emitted for it. An image has
    /// no payload here, so its unrelated `image.missing-file` is dropped.
    fn normalized_roundness_of(
        kind: &str,
        element_fields: &str,
    ) -> (Option<NormalizedRoundness>, Vec<(DiagnosticCode, String)>) {
        let (element, mut diagnostics) = normalized_single_element(&format!(
            r##"{{"id":"element","type":"{kind}","width":120,"height":80,"seed":1,"updated":1,{element_fields}}}"##
        ));
        if kind == "image" {
            diagnostics.retain(|(code, _)| *code != DiagnosticCode::IMAGE_MISSING_FILE);
        }
        (element.roundness, diagnostics)
    }

    #[test]
    fn legacy_round_migrates_for_every_kind_whose_roundness_the_manifest_reads() {
        // restoreElementWithProperties: no `roundness` and
        // `strokeSharpness === "round"` gives the isUsingAdaptiveRadius kinds
        // ROUNDNESS.LEGACY (1) and every other kind PROPORTIONAL_RADIUS (2).
        // getCornerRadius treats both as the proportional corner radius, so
        // the migration is kind-independent once it reaches the rendering view.
        for kind in [
            "rectangle",
            "image",
            "embeddable",
            "iframe",
            "diamond",
            "line",
            "arrow",
        ] {
            let (roundness, diagnostics) =
                normalized_roundness_of(kind, r##""strokeSharpness":"round""##);
            assert_eq!(roundness, Some(NormalizedRoundness::Proportional), "{kind}");
            assert!(diagnostics.is_empty(), "{kind}: {diagnostics:?}");
        }

        // Kinds whose `/roundness` the revision-1 manifest ignores keep
        // ignoring the migrated record as well.
        for kind in ["ellipse", "text", "freedraw", "frame", "magicframe"] {
            let (roundness, diagnostics) =
                normalized_roundness_of(kind, r##""strokeSharpness":"round""##);
            assert_eq!(roundness, None, "{kind}");
            assert!(diagnostics.is_empty(), "{kind}: {diagnostics:?}");
        }
    }

    #[test]
    fn legacy_sharp_and_unrecognized_stroke_sharpness_round_nothing() {
        // The restore compares `strokeSharpness === "round"` strictly; every
        // other string leaves `roundness: null`. The manifest admits any
        // string at `/strokeSharpness`, so neither value is diagnosed here.
        for value in [r##""sharp""##, r##""ROUND""##] {
            let (roundness, diagnostics) =
                normalized_roundness_of("rectangle", &format!(r##""strokeSharpness":{value}"##));
            assert_eq!(roundness, None, "{value}");
            assert!(diagnostics.is_empty(), "{value}: {diagnostics:?}");
        }

        // A non-string value takes the manifest's `unsupported-property-value`
        // substitution at `/strokeSharpness` and likewise rounds nothing.
        let (roundness, diagnostics) =
            normalized_roundness_of("rectangle", r##""strokeSharpness":7"##);
        assert_eq!(roundness, None);
        assert_eq!(
            diagnostics,
            [(
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                "/elements/0/strokeSharpness".to_owned(),
            )]
        );
    }

    #[test]
    fn an_explicit_roundness_record_wins_over_legacy_stroke_sharpness() {
        // `element.roundness ? element.roundness : ...` — any record object,
        // even one that resolves to no rounding, takes precedence; only a
        // missing or `null` roundness consults the legacy value.
        for (fields, expected) in [
            (
                r##""roundness":{"type":3,"value":8},"strokeSharpness":"round""##,
                Some(NormalizedRoundness::Adaptive { radius: 8.0 }),
            ),
            (
                r##""roundness":{"type":2},"strokeSharpness":"sharp""##,
                Some(NormalizedRoundness::Proportional),
            ),
            (r##""roundness":{},"strokeSharpness":"round""##, None),
            (
                r##""roundness":null,"strokeSharpness":"round""##,
                Some(NormalizedRoundness::Proportional),
            ),
            (r##""roundness":null,"strokeSharpness":"sharp""##, None),
        ] {
            let (roundness, diagnostics) = normalized_roundness_of("rectangle", fields);
            assert_eq!(roundness, expected, "{fields}");
            assert!(diagnostics.is_empty(), "{fields}: {diagnostics:?}");
        }
    }

    #[test]
    fn an_invalid_roundness_value_is_diagnosed_and_never_consults_stroke_sharpness() {
        // The reference keeps a truthy malformed `roundness` verbatim, which
        // then rounds nothing; revision 1 substitutes the same sharp result
        // for every malformed value and reports it at `/roundness`.
        let (roundness, diagnostics) =
            normalized_roundness_of("rectangle", r##""roundness":5,"strokeSharpness":"round""##);
        assert_eq!(roundness, None);
        assert_eq!(
            diagnostics,
            [(
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                "/elements/0/roundness".to_owned(),
            )]
        );
    }
}
