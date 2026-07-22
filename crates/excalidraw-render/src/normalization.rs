use std::collections::TryReserveError;

use sha2::{Digest, Sha256};

use crate::outcome::DiagnosticOrder;
use crate::scene::{
    pointer_child, pointer_index, BindingSource, ElementKind, ElementSource, FrameRenderingSource,
    ImageCropSource, NumberArraySource, PointArraySource, SourceField,
};
use crate::{
    CompatibilityDiagnostic, CompatibilityRevision, DiagnosticCode, DiagnosticEffect,
    DiagnosticSeverity, ErrorDetails, OutputFormat, Recovery, SceneDocument, SupplementalAssets,
};

const MISSING_ID_DOMAIN: &[u8] = b"excalidraw-render:missing-element-id:v1\0";
const DUPLICATE_ID_DOMAIN: &[u8] = b"excalidraw-render:duplicate-element-id:v1\0";
const ID_COLLISION_DOMAIN: &[u8] = b"excalidraw-render:element-id-collision:v1\0";
const ROUGH_SEED_DOMAIN: &[u8] = b"excalidraw-render:rough-seed:v1\0";
const NORMALIZED_FIELD_COUNT: u64 = 14;
const SCALAR_REFERENCE_FIELD_COUNT: u64 = 10;
const GENERATED_ID_BYTES: usize = "generated-".len() + 64;
pub(crate) const NORMALIZATION_WORK_UNITS: u64 = NORMALIZED_FIELD_COUNT * 8;

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

#[derive(Debug, PartialEq)]
pub(crate) enum NormalizedElementKind {
    Rectangle,
    Diamond,
    Ellipse,
    Linear(NormalizedLinear),
    Freedraw(NormalizedFreedraw),
    Image(NormalizedImage),
    Frame(NormalizedFrame),
    MagicFrame(NormalizedFrame),
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub(crate) struct NormalizedBinding {
    pub(crate) element_id: String,
    pub(crate) focus: f64,
    pub(crate) gap: f64,
    pub(crate) fixed_point: Option<[f64; 2]>,
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub(crate) struct NormalizedFileRecord {
    pub(crate) key: String,
    pub(crate) id: String,
    pub(crate) created: Option<u64>,
    pub(crate) source_pointer: String,
    pub(crate) source_order: u64,
    pub(crate) normalized_working_bytes: u64,
    normalization_work_units: u64,
}

pub(crate) struct NormalizedElementSource {
    id: String,
    pointer: String,
    coordinate_positions: [usize; 4],
}

impl NormalizedElementSource {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn pointer(&self) -> &str {
        &self.pointer
    }

    pub(crate) fn coordinate_position(&self, field: &str) -> u64 {
        let position = match field {
            "x" => self.coordinate_positions[0],
            "y" => self.coordinate_positions[1],
            "width" => self.coordinate_positions[2],
            "height" => self.coordinate_positions[3],
            _ => usize::MAX,
        };
        u64::try_from(position).unwrap_or(u64::MAX)
    }
}

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

    fn as_u64(self) -> u64 {
        u64::try_from(self.member)
            .unwrap_or(u64::MAX)
            .saturating_mul(u64::from(u32::MAX) + 1)
            .saturating_add(u64::try_from(self.nested).unwrap_or(u64::MAX))
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
        ) && matches!(defaulted(source.group_ids, 0), Some(0))
            && matches!(defaulted(source.frame_id, None), Some(None))
            && !matches!(source.bound_elements, SourceField::Invalid)
            && matches!(defaulted(source.link, None), Some(None));
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
    if !fractional_indices_are_strictly_ascending(&source_indices) {
        for index in &mut source_indices {
            if index.is_some_and(|index| !is_valid_fractional_key(index)) {
                *index = None;
            }
        }
    }
    let normalized_indices = repair_fractional_indices(&source_indices)?;

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
            .saturating_add(planned_kind_working_bytes(&source, participation));
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
                SourceField::Value(Some(link)) if crate::scene_surface::safe_link(link) => {
                    Some(memory_at_source!(try_owned(link)))
                }
                SourceField::Missing
                | SourceField::Invalid
                | SourceField::Value(None | Some(_)) => None,
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

impl ElementKind {
    const fn normalization_event_name(self) -> &'static str {
        match self {
            Self::Rectangle => "normalized_rectangle",
            Self::Diamond => "normalized_diamond",
            Self::Ellipse => "normalized_ellipse",
            Self::Line | Self::LegacyDraw | Self::Arrow => "normalized_linear",
            Self::Freedraw => "normalized_freedraw",
            Self::Image => "normalized_image",
            Self::Frame | Self::MagicFrame => "normalized_frame",
        }
    }

    fn normalized_field_count(self, participation: NormalizationParticipation) -> u64 {
        match self {
            Self::Rectangle | Self::Diamond | Self::Ellipse => 0,
            Self::Line | Self::LegacyDraw => 6,
            Self::Arrow => 7,
            Self::Freedraw => 4,
            Self::Image => 3 + u64::from(participation.authoritative_file),
            Self::Frame | Self::MagicFrame => participation.frame_names as u64,
        }
    }
}

fn planned_kind_working_bytes(
    source: &ElementSource<'_>,
    participation: NormalizationParticipation,
) -> u64 {
    let optional_owned_bytes =
        |value: Option<&str>| value.map_or(64, |value| owned_field_bytes(value.len()));
    match source.kind {
        ElementKind::Rectangle | ElementKind::Diamond | ElementKind::Ellipse => 0,
        ElementKind::Line | ElementKind::LegacyDraw | ElementKind::Arrow => {
            let point_count = match source.points {
                SourceField::Value(points)
                    if points.len() >= 2
                        && (0..points.len())
                            .all(|index| matches!(points.point(index), SourceField::Value(_))) =>
                {
                    points.len()
                }
                SourceField::Missing | SourceField::Invalid | SourceField::Value(_) => 2,
            };
            let arrow = matches!(source.kind, ElementKind::Arrow);
            let elbowed = arrow && defaulted(source.elbowed, false).unwrap_or(false);
            u64::try_from(point_count)
                .unwrap_or(u64::MAX)
                .saturating_mul(2 * 64)
                .saturating_add(3 * 64)
                .saturating_add(optional_owned_bytes(planned_arrowhead(
                    source.start_arrowhead,
                    None,
                )))
                .saturating_add(optional_owned_bytes(planned_arrowhead(
                    source.end_arrowhead,
                    arrow.then_some("arrow"),
                )))
                .saturating_add(u64::from(arrow) * 64)
                .saturating_add(planned_binding_working_bytes(source.start_binding, elbowed))
                .saturating_add(planned_binding_working_bytes(source.end_binding, elbowed))
        }
        ElementKind::Freedraw => {
            let point_count = match source.points {
                SourceField::Value(points)
                    if (0..points.len())
                        .all(|index| matches!(points.point(index), SourceField::Value(_))) =>
                {
                    points.len()
                }
                SourceField::Missing | SourceField::Invalid | SourceField::Value(_) => 0,
            };
            let simulate_pressure = defaulted(source.simulate_pressure, true).unwrap_or(true);
            let source_point_count = match source.points {
                SourceField::Value(points) => points.len(),
                SourceField::Missing | SourceField::Invalid => point_count,
            };
            let pressure_count = match source.pressures {
                SourceField::Value(pressures)
                    if pressures_are_valid(
                        source.pressures,
                        source_point_count,
                        simulate_pressure,
                    ) =>
                {
                    pressures.len()
                }
                SourceField::Missing | SourceField::Invalid | SourceField::Value(_) => 0,
            };
            u64::try_from(point_count)
                .unwrap_or(u64::MAX)
                .saturating_mul(2 * 64)
                .saturating_add(
                    u64::try_from(pressure_count)
                        .unwrap_or(u64::MAX)
                        .saturating_mul(64),
                )
                .saturating_add(2 * 64)
        }
        ElementKind::Image => {
            let file_id = match source.file_id {
                SourceField::Value(Some(file_id)) => Some(file_id),
                SourceField::Missing | SourceField::Invalid | SourceField::Value(None) => None,
            };
            optional_owned_bytes(file_id)
                .saturating_add(2 * 64)
                .saturating_add(u64::from(participation.authoritative_file) * 64)
                .saturating_add(
                    u64::from(
                        participation.authoritative_file
                            && normalized_image_crop(source.image_crop).is_some(),
                    ) * 6
                        * 64,
                )
        }
        ElementKind::Frame | ElementKind::MagicFrame if participation.frame_names => {
            let name = match source.name {
                SourceField::Value(Some(name)) => Some(name),
                SourceField::Missing | SourceField::Invalid | SourceField::Value(None) => None,
            };
            optional_owned_bytes(name)
        }
        ElementKind::Frame | ElementKind::MagicFrame => 0,
    }
}

fn planned_arrowhead<'a>(
    source: SourceField<Option<&'a str>>,
    default: Option<&'a str>,
) -> Option<&'a str> {
    match source {
        SourceField::Value(Some(value)) if is_arrowhead(value) => Some(value),
        SourceField::Value(None) => None,
        SourceField::Missing | SourceField::Invalid | SourceField::Value(Some(_)) => default,
    }
}

fn planned_binding_working_bytes(
    source: SourceField<Option<BindingSource<'_>>>,
    elbowed: bool,
) -> u64 {
    let SourceField::Value(Some(source)) = source else {
        return 0;
    };
    let fixed_point_bytes = if elbowed {
        match source.fixed_point() {
            SourceField::Missing => return 0,
            SourceField::Value(_) | SourceField::Invalid => 2 * 64,
        }
    } else {
        0
    };
    let SourceField::Value(element_id) = source.element_id() else {
        return 0;
    };
    if element_id.is_empty()
        || !matches!(source.gap(), SourceField::Value(_))
        || matches!(source.focus(), SourceField::Invalid)
    {
        return 0;
    }
    owned_field_bytes(element_id.len())
        .saturating_add(2 * 64)
        .saturating_add(fixed_point_bytes)
}

fn kind_working_bytes(
    kind: &NormalizedElementKind,
    participation: NormalizationParticipation,
) -> u64 {
    let optional_owned_bytes =
        |value: Option<&str>| value.map_or(64, |value| owned_field_bytes(value.len()));
    let binding_bytes = |binding: Option<&NormalizedBinding>| {
        binding.map_or(0, |binding| {
            owned_field_bytes(binding.element_id.len())
                .saturating_add(2 * 64)
                .saturating_add(u64::from(binding.fixed_point.is_some()) * 2 * 64)
        })
    };
    match kind {
        NormalizedElementKind::Rectangle
        | NormalizedElementKind::Diamond
        | NormalizedElementKind::Ellipse => 0,
        NormalizedElementKind::Linear(linear) => u64::try_from(linear.points.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(2 * 64)
            .saturating_add(3 * 64)
            .saturating_add(optional_owned_bytes(linear.start_arrowhead.as_deref()))
            .saturating_add(optional_owned_bytes(linear.end_arrowhead.as_deref()))
            .saturating_add(u64::from(matches!(linear.kind, NormalizedLinearKind::Arrow)) * 64)
            .saturating_add(binding_bytes(linear.start_binding.as_ref()))
            .saturating_add(binding_bytes(linear.end_binding.as_ref())),
        NormalizedElementKind::Freedraw(freedraw) => u64::try_from(freedraw.points.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(2 * 64)
            .saturating_add(
                u64::try_from(freedraw.pressures.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(64),
            )
            .saturating_add(2 * 64),
        NormalizedElementKind::Image(image) => optional_owned_bytes(image.file_id.as_deref())
            .saturating_add(2 * 64)
            .saturating_add(u64::from(participation.authoritative_file) * 64)
            .saturating_add(u64::from(image.crop.is_some()) * 6 * 64),
        NormalizedElementKind::Frame(frame) | NormalizedElementKind::MagicFrame(frame) => {
            if participation.frame_names {
                optional_owned_bytes(frame.name.as_deref())
            } else {
                0
            }
        }
    }
}

fn element_fields<'a>(
    source: &ElementSource<'a>,
    policy: &'static crate::scene_surface::SceneSurfacePolicy,
) -> Option<CommonFields<'a>> {
    let ignored = |path| {
        crate::scene_surface::element_ignores_inherited_path(
            policy,
            source.kind.scene_surface_kind(),
            path,
        )
    };
    let mut x = manifested_value(source.x, 0.0, |_| true);
    let mut y = manifested_value(source.y, 0.0, |_| true);
    let mut width = manifested_value(source.width, 0.0, |_| true);
    let mut height = manifested_value(source.height, 0.0, |_| true);
    if !matches!(
        source.kind,
        ElementKind::Line | ElementKind::LegacyDraw | ElementKind::Arrow
    ) {
        if width < 0.0 {
            x += width;
        }
        width = width.abs();
        if height < 0.0 {
            y += height;
        }
        height = height.abs();
    }
    Some(CommonFields {
        x: canonical_zero(x),
        y: canonical_zero(y),
        width: canonical_zero(width),
        height: canonical_zero(height),
        angle: canonical_zero(manifested_value(source.angle, 0.0, |_| true)),
        stroke_color: if ignored("/strokeColor") {
            "#1e1e1e"
        } else {
            manifested_value(source.stroke_color, "#1e1e1e", |value| {
                crate::scene_surface::css_color_is_admitted(value)
            })
        },
        background_color: if ignored("/backgroundColor") {
            "transparent"
        } else {
            manifested_value(source.background_color, "transparent", |value| {
                crate::scene_surface::css_color_is_admitted(value)
            })
        },
        fill_style: if ignored("/fillStyle") {
            "solid"
        } else {
            manifested_value(source.fill_style, "solid", |value| {
                matches!(value, "hachure" | "cross-hatch" | "solid" | "zigzag")
            })
        },
        stroke_width: if ignored("/strokeWidth") {
            2.0
        } else {
            manifested_value(source.stroke_width, 2.0, |value| value > 0.0)
        },
        stroke_style: if ignored("/strokeStyle") {
            "solid"
        } else {
            manifested_value(source.stroke_style, "solid", |value| {
                matches!(value, "solid" | "dashed" | "dotted")
            })
        },
        roughness: if ignored("/roughness") {
            1.0
        } else {
            manifested_value(source.roughness, 1.0, |value| value >= 0.0)
        },
        opacity: manifested_value(source.opacity, 100.0, |value| {
            (0.0..=100.0).contains(&value)
        }),
        source_seed: if ignored("/seed") {
            1
        } else {
            manifested_value(source.seed, 1, |_| true)
        },
    })
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

#[allow(clippy::too_many_arguments)]
fn normalize_element_kind(
    source: &ElementSource<'_>,
    fields: &CommonFields<'_>,
    policy: &'static crate::scene_surface::SceneSurfacePolicy,
    element_pointer: &str,
    element_id: &str,
    format: OutputFormat,
    participation: NormalizationParticipation,
    diagnostic_limit: usize,
    diagnostics: &mut Vec<ElementDiagnostic>,
) -> Result<(NormalizedElementKind, f64, f64, f64, f64), TryReserveError> {
    let mut diagnostic_context = ElementDiagnosticContext {
        diagnostics,
        source,
        element_pointer,
        element_id,
        format,
        policy,
        diagnostic_limit,
    };
    macro_rules! diagnose_common {
        ($field:literal, $invalid:expr) => {
            if $invalid
                && !crate::scene_surface::element_ignores_inherited_path(
                    policy,
                    source.kind.scene_surface_kind(),
                    concat!("/", $field),
                )
            {
                diagnostic_context.unsupported($field, &[], Recovery::Substituted, None)?;
            }
        };
    }
    diagnose_common!("x", matches!(source.x, SourceField::Invalid));
    diagnose_common!("y", matches!(source.y, SourceField::Invalid));
    diagnose_common!("width", matches!(source.width, SourceField::Invalid));
    diagnose_common!("height", matches!(source.height, SourceField::Invalid));
    diagnose_common!("angle", matches!(source.angle, SourceField::Invalid));
    diagnose_common!(
        "fillStyle",
        !matches!(
            source.fill_style,
            SourceField::Missing
                | SourceField::Value("hachure" | "cross-hatch" | "solid" | "zigzag")
        )
    );
    diagnose_common!(
        "strokeWidth",
        !source_accepts(source.stroke_width, |value| value > 0.0)
    );
    diagnose_common!(
        "strokeStyle",
        !matches!(
            source.stroke_style,
            SourceField::Missing | SourceField::Value("solid" | "dashed" | "dotted")
        )
    );
    diagnose_common!(
        "roughness",
        !source_accepts(source.roughness, |value| value >= 0.0)
    );
    diagnose_common!(
        "opacity",
        !source_accepts(source.opacity, |value| (0.0..=100.0).contains(&value))
    );
    diagnose_common!("seed", matches!(source.seed, SourceField::Invalid));
    diagnose_common!(
        "isDeleted",
        matches!(source.is_deleted, SourceField::Invalid)
    );
    diagnose_common!("frameId", matches!(source.frame_id, SourceField::Invalid));
    let roundness_ignored = crate::scene_surface::element_ignores_inherited_path(
        policy,
        source.kind.scene_surface_kind(),
        "/roundness",
    );
    if !roundness_ignored {
        match source.roundness {
            SourceField::Invalid => diagnostic_context.unsupported(
                "roundness",
                &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                Recovery::Substituted,
                None,
            )?,
            SourceField::Value(Some(roundness)) => {
                for (position, member) in roundness.members().iter().enumerate() {
                    let semantic_path = pointer_child("", &member.key)?;
                    if !crate::scene_surface::is_manifested_path(
                        policy,
                        "nested.roundness",
                        &semantic_path,
                    ) {
                        diagnostic_context.unsupported_property_nested(
                            "roundness",
                            &member.key,
                            position,
                        )?;
                    }
                }
                if !matches!(
                    roundness.kind(),
                    SourceField::Missing | SourceField::Value(1..=3)
                ) {
                    diagnostic_context.unsupported_nested(
                        "roundness",
                        "type",
                        roundness.member_position("type"),
                        None,
                        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                        Recovery::Substituted,
                    )?;
                }
                let value_is_valid = match roundness.value() {
                    SourceField::Missing => true,
                    SourceField::Value(value) => value >= 0.0,
                    SourceField::Invalid => false,
                };
                if !value_is_valid {
                    diagnostic_context.unsupported_nested(
                        "roundness",
                        "value",
                        roundness.member_position("value"),
                        None,
                        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                        Recovery::Substituted,
                    )?;
                }
            }
            SourceField::Missing | SourceField::Value(None) => {}
        }
    }
    match source.kind {
        ElementKind::Rectangle => Ok((
            NormalizedElementKind::Rectangle,
            fields.x,
            fields.y,
            fields.width,
            fields.height,
        )),
        ElementKind::Diamond => Ok((
            NormalizedElementKind::Diamond,
            fields.x,
            fields.y,
            fields.width,
            fields.height,
        )),
        ElementKind::Ellipse => Ok((
            NormalizedElementKind::Ellipse,
            fields.x,
            fields.y,
            fields.width,
            fields.height,
        )),
        ElementKind::Line | ElementKind::LegacyDraw | ElementKind::Arrow => {
            if matches!(source.points, SourceField::Invalid) {
                diagnostic_context.unsupported(
                    "points",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            let source_points = match source.points {
                SourceField::Value(points) if points.len() >= 2 => Some(points),
                SourceField::Missing | SourceField::Value(_) | SourceField::Invalid => None,
            };
            let point_count = source_points.map_or(2, |points| points.len());
            let mut points = Vec::new();
            let mut geometry_available = true;
            points.try_reserve_exact(point_count)?;
            if let Some(source_points) = source_points {
                for index in 0..source_points.len() {
                    match source_points.point(index) {
                        SourceField::Value(point) => {
                            points.push(point);
                            for component in 2..source_points.component_count(index).unwrap_or(2) {
                                let pointer = pointer_index(
                                    &pointer_index(
                                        &pointer_child(element_pointer, "points")?,
                                        index,
                                    )?,
                                    component,
                                )?;
                                diagnostic_context.unsupported_property_at(
                                    "points",
                                    nested_path_ordinal(index, component),
                                    &pointer,
                                )?;
                            }
                        }
                        SourceField::Missing | SourceField::Invalid => {
                            geometry_available = false;
                            let mut pointer =
                                pointer_index(&pointer_child(element_pointer, "points")?, index)?;
                            if let Some(component) = source_points.invalid_component(index) {
                                pointer = pointer_index(&pointer, component)?;
                            }
                            if diagnostic_context.can_retain() {
                                diagnostic_context.diagnostics.try_reserve(1)?;
                                diagnostic_context.diagnostics.push((
                                    SourceDiagnosticPosition::nested(
                                        source.member_position("points").unwrap_or(usize::MAX),
                                        nested_path_ordinal(
                                            index,
                                            source_points.invalid_component(index).unwrap_or(0),
                                        ),
                                    ),
                                    compatibility_diagnostic(
                                        DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                                        format,
                                        &[DiagnosticEffect::Visual],
                                        Some(element_id),
                                        &pointer,
                                        "malformed linear point used the known-element fallback",
                                        DiagnosticRecovery::new(
                                            Recovery::Placeholder,
                                            Some("renderer.known-element-unavailable.v1"),
                                        ),
                                    )?,
                                ));
                            }
                            points.clear();
                            points.push([0.0, 0.0]);
                            points.push([fields.width, fields.height]);
                            break;
                        }
                    }
                }
            } else {
                points.push([0.0, 0.0]);
                points.push([fields.width, fields.height]);
            }

            let offset = points[0];
            let mut x = fields.x;
            let mut y = fields.y;
            if offset != [0.0, 0.0] {
                x = canonical_zero(x + offset[0]);
                y = canonical_zero(y + offset[1]);
                for point in &mut points {
                    point[0] = canonical_zero(point[0] - offset[0]);
                    point[1] = canonical_zero(point[1] - offset[1]);
                }
            }
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for point in &points {
                min_x = min_x.min(point[0]);
                min_y = min_y.min(point[1]);
                max_x = max_x.max(point[0]);
                max_y = max_y.max(point[1]);
            }
            let width = canonical_zero(max_x - min_x);
            let height = canonical_zero(max_y - min_y);
            let kind = if matches!(source.kind, ElementKind::Arrow) {
                NormalizedLinearKind::Arrow
            } else {
                NormalizedLinearKind::Line
            };
            if matches!(source.kind, ElementKind::Arrow)
                && matches!(source.elbowed, SourceField::Invalid)
            {
                diagnostic_context.unsupported(
                    "elbowed",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            for (field, arrowhead) in [
                ("startArrowhead", source.start_arrowhead),
                ("endArrowhead", source.end_arrowhead),
            ] {
                if !arrowhead_is_valid_or_missing(arrowhead) {
                    diagnostic_context.unsupported(
                        field,
                        &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                        Recovery::Substituted,
                        None,
                    )?;
                }
            }
            let elbowed = matches!(source.kind, ElementKind::Arrow)
                && defaulted(source.elbowed, false).unwrap_or(false);
            let start_binding = normalize_binding(
                source.start_binding,
                elbowed,
                "startBinding",
                policy,
                &mut diagnostic_context,
            )?;
            let end_binding = normalize_binding(
                source.end_binding,
                elbowed,
                "endBinding",
                policy,
                &mut diagnostic_context,
            )?;
            let start_arrowhead = normalized_arrowhead(source.start_arrowhead, None)?;
            let end_arrowhead = normalized_arrowhead(
                source.end_arrowhead,
                matches!(kind, NormalizedLinearKind::Arrow).then_some("arrow"),
            )?;
            Ok((
                NormalizedElementKind::Linear(NormalizedLinear {
                    kind,
                    points,
                    start_binding,
                    end_binding,
                    last_committed_point: None,
                    start_arrowhead,
                    end_arrowhead,
                    elbowed,
                    geometry_available,
                }),
                x,
                y,
                width,
                height,
            ))
        }
        ElementKind::Freedraw => {
            if matches!(source.points, SourceField::Invalid) {
                diagnostic_context.unsupported(
                    "points",
                    &[DiagnosticEffect::Visual],
                    Recovery::Placeholder,
                    Some("renderer.known-element-unavailable.v1"),
                )?;
            }
            if matches!(source.simulate_pressure, SourceField::Invalid) {
                diagnostic_context.unsupported(
                    "simulatePressure",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            let (points, geometry_available) = normalized_freedraw_points(source.points)?;
            if let SourceField::Value(source_points) = source.points {
                for index in 0..source_points.len() {
                    if !matches!(source_points.point(index), SourceField::Value(_)) {
                        continue;
                    }
                    for component in 2..source_points.component_count(index).unwrap_or(2) {
                        let pointer = pointer_index(
                            &pointer_index(&pointer_child(element_pointer, "points")?, index)?,
                            component,
                        )?;
                        diagnostic_context.unsupported_property_at(
                            "points",
                            nested_path_ordinal(index, component),
                            &pointer,
                        )?;
                    }
                }
            }
            if !geometry_available {
                if let SourceField::Value(source_points) = source.points {
                    if let Some((index, component)) = source_points.first_invalid_point() {
                        let mut pointer =
                            pointer_index(&pointer_child(element_pointer, "points")?, index)?;
                        if let Some(component) = component {
                            pointer = pointer_index(&pointer, component)?;
                        }
                        if diagnostic_context.can_retain() {
                            diagnostic_context.diagnostics.try_reserve(1)?;
                            diagnostic_context.diagnostics.push((
                                SourceDiagnosticPosition::nested(
                                    source.member_position("points").unwrap_or(usize::MAX),
                                    nested_path_ordinal(index, component.unwrap_or(0)),
                                ),
                                compatibility_diagnostic(
                                    DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                                    format,
                                    &[DiagnosticEffect::Visual],
                                    Some(element_id),
                                    &pointer,
                                    "malformed freedraw point used the known-element fallback",
                                    DiagnosticRecovery::new(
                                        Recovery::Placeholder,
                                        Some("renderer.known-element-unavailable.v1"),
                                    ),
                                )?,
                            ));
                        }
                    }
                }
            }
            let simulate_pressure = defaulted(source.simulate_pressure, true).unwrap_or(true);
            let source_point_count = match source.points {
                SourceField::Value(source_points) => source_points.len(),
                SourceField::Missing | SourceField::Invalid => points.len(),
            };
            if !pressures_are_valid(source.pressures, source_point_count, simulate_pressure) {
                diagnostic_context.unsupported(
                    "pressures",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            let pressures =
                normalized_pressures(source.pressures, source_point_count, simulate_pressure)?;
            Ok((
                NormalizedElementKind::Freedraw(NormalizedFreedraw {
                    points,
                    pressures,
                    simulate_pressure,
                    last_committed_point: None,
                    geometry_available,
                }),
                fields.x,
                fields.y,
                fields.width,
                fields.height,
            ))
        }
        ElementKind::Image => {
            if matches!(source.file_id, SourceField::Invalid) {
                diagnostic_context.unsupported(
                    "fileId",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            if !matches!(
                source.image_status,
                SourceField::Missing | SourceField::Value("pending" | "saved" | "error")
            ) {
                diagnostic_context.unsupported(
                    "status",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            if !image_scale_is_valid(source.image_scale) {
                diagnostic_context.unsupported(
                    "scale",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            if matches!(source.image_crop, SourceField::Invalid) {
                diagnostic_context.unsupported(
                    "crop",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            } else if participation.authoritative_file {
                if let SourceField::Value(Some(crop)) = source.image_crop {
                    diagnose_invalid_crop_members(&mut diagnostic_context, crop)?;
                    for (position, member) in crop.members().iter().enumerate() {
                        let semantic_path = pointer_child("", &member.key)?;
                        if !crate::scene_surface::is_manifested_path(
                            policy,
                            "nested.image_crop",
                            &semantic_path,
                        ) {
                            diagnostic_context.unsupported_property_nested(
                                "crop",
                                &member.key,
                                position,
                            )?;
                        }
                    }
                }
            }
            let status = match source.image_status {
                SourceField::Value("saved") => NormalizedImageStatus::Saved,
                SourceField::Value("error") => NormalizedImageStatus::Error,
                SourceField::Missing
                | SourceField::Invalid
                | SourceField::Value("pending")
                | SourceField::Value(_) => NormalizedImageStatus::Pending,
            };
            let file_id = match source.file_id {
                SourceField::Value(Some(file_id)) => Some(file_id),
                SourceField::Missing | SourceField::Invalid | SourceField::Value(None) => None,
            };
            if !matches!(source.file_id, SourceField::Invalid)
                && (!participation.authoritative_file || file_id.is_none())
            {
                diagnostic_context.missing_image_file(file_id, status)?;
            }
            Ok((
                NormalizedElementKind::Image(NormalizedImage {
                    file_id: file_id.map(try_owned).transpose()?,
                    status,
                    scale: normalized_image_scale(source.image_scale),
                    crop: participation
                        .authoritative_file
                        .then(|| normalized_image_crop(source.image_crop))
                        .flatten(),
                }),
                fields.x,
                fields.y,
                fields.width,
                fields.height,
            ))
        }
        ElementKind::Frame => {
            if participation.frame_names && matches!(source.name, SourceField::Invalid) {
                diagnostic_context.unsupported(
                    "name",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            Ok((
                NormalizedElementKind::Frame(NormalizedFrame {
                    name: if participation.frame_names {
                        normalized_optional_string(source.name)?
                    } else {
                        None
                    },
                }),
                fields.x,
                fields.y,
                fields.width,
                fields.height,
            ))
        }
        ElementKind::MagicFrame => {
            if participation.frame_names && matches!(source.name, SourceField::Invalid) {
                diagnostic_context.unsupported(
                    "name",
                    &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                    Recovery::Substituted,
                    None,
                )?;
            }
            Ok((
                NormalizedElementKind::MagicFrame(NormalizedFrame {
                    name: if participation.frame_names {
                        normalized_optional_string(source.name)?
                    } else {
                        None
                    },
                }),
                fields.x,
                fields.y,
                fields.width,
                fields.height,
            ))
        }
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

fn diagnose_invalid_crop_members(
    context: &mut ElementDiagnosticContext<'_, '_>,
    crop: ImageCropSource<'_>,
) -> Result<(), TryReserveError> {
    let crop_pointer = pointer_child(context.element_pointer, "crop")?;
    let top_position = context
        .source
        .member_position("crop")
        .unwrap_or(usize::MAX / 2);
    for (manifest_ordinal, (field, accepts_zero)) in [
        ("x", true),
        ("y", true),
        ("width", false),
        ("height", false),
        ("naturalWidth", false),
        ("naturalHeight", false),
    ]
    .into_iter()
    .enumerate()
    {
        if !context.can_retain() {
            break;
        }
        let value = crop.number(field);
        let valid = matches!(value, SourceField::Value(value) if value > 0.0 || (accepts_zero && value == 0.0));
        if valid {
            continue;
        }
        let pointer = pointer_child(&crop_pointer, field)?;
        context.diagnostics.try_reserve(1)?;
        let position = crop.member_position(field).map_or(
            SourceDiagnosticPosition::nested(
                top_position,
                crop.len().saturating_add(manifest_ordinal),
            ),
            |nested| SourceDiagnosticPosition::nested(top_position, nested),
        );
        context.diagnostics.push((
            position,
            compatibility_diagnostic(
                DiagnosticCode::COMPATIBILITY_UNSUPPORTED_PROPERTY_VALUE,
                context.format,
                &[DiagnosticEffect::Visual],
                Some(context.element_id),
                &pointer,
                "invalid image crop member disabled crop rendering",
                DiagnosticRecovery::new(Recovery::Omitted, None),
            )?,
        ));
    }
    Ok(())
}

fn normalized_freedraw_points(
    source: SourceField<PointArraySource<'_>>,
) -> Result<(Vec<[f64; 2]>, bool), TryReserveError> {
    let SourceField::Value(source) = source else {
        return Ok((Vec::new(), !matches!(source, SourceField::Invalid)));
    };
    let mut points = Vec::new();
    points.try_reserve_exact(source.len())?;
    for index in 0..source.len() {
        let SourceField::Value(point) = source.point(index) else {
            return Ok((Vec::new(), false));
        };
        points.push([canonical_zero(point[0]), canonical_zero(point[1])]);
    }
    Ok((points, true))
}

fn normalized_pressures(
    source: SourceField<NumberArraySource<'_>>,
    point_count: usize,
    simulate_pressure: bool,
) -> Result<Vec<f64>, TryReserveError> {
    let SourceField::Value(source) = source else {
        return Ok(Vec::new());
    };
    if !simulate_pressure && source.len() != point_count {
        return Ok(Vec::new());
    }
    let mut pressures = Vec::new();
    pressures.try_reserve_exact(source.len())?;
    for index in 0..source.len() {
        let SourceField::Value(pressure) = source.number(index) else {
            return Ok(Vec::new());
        };
        if !(0.0..=1.0).contains(&pressure) {
            return Ok(Vec::new());
        }
        pressures.push(canonical_zero(pressure));
    }
    Ok(pressures)
}

fn pressures_are_valid(
    source: SourceField<NumberArraySource<'_>>,
    point_count: usize,
    simulate_pressure: bool,
) -> bool {
    match source {
        SourceField::Missing => true,
        SourceField::Invalid => false,
        SourceField::Value(source) => {
            (simulate_pressure || source.len() == point_count)
                && (0..source.len()).all(|index| {
                    matches!(source.number(index), SourceField::Value(value) if (0.0..=1.0).contains(&value))
                })
        }
    }
}

fn normalized_optional_string(
    source: SourceField<Option<&str>>,
) -> Result<Option<String>, TryReserveError> {
    match source {
        SourceField::Value(Some(value)) => Ok(Some(try_owned(value)?)),
        SourceField::Missing | SourceField::Invalid | SourceField::Value(None) => Ok(None),
    }
}

fn normalized_image_scale(source: SourceField<NumberArraySource<'_>>) -> [i8; 2] {
    let SourceField::Value(values) = source else {
        return [1, 1];
    };
    if values.len() != 2 {
        return [1, 1];
    }
    let component = |index| match values.number(index) {
        SourceField::Value(-1.0) => Some(-1),
        SourceField::Value(1.0) => Some(1),
        SourceField::Missing | SourceField::Invalid | SourceField::Value(_) => None,
    };
    match (component(0), component(1)) {
        (Some(x), Some(y)) => [x, y],
        _ => [1, 1],
    }
}

fn image_scale_is_valid(source: SourceField<NumberArraySource<'_>>) -> bool {
    match source {
        SourceField::Missing => true,
        SourceField::Invalid => false,
        SourceField::Value(values) => {
            values.len() == 2
                && [0, 1]
                    .into_iter()
                    .all(|index| matches!(values.number(index), SourceField::Value(-1.0 | 1.0)))
        }
    }
}

fn normalized_image_crop(
    source: SourceField<Option<ImageCropSource<'_>>>,
) -> Option<NormalizedImageCrop> {
    let SourceField::Value(Some(source)) = source else {
        return None;
    };
    let number = |key| match source.number(key) {
        SourceField::Value(value) => Some(value),
        SourceField::Missing | SourceField::Invalid => None,
    };
    let crop = NormalizedImageCrop {
        x: number("x")?,
        y: number("y")?,
        width: number("width")?,
        height: number("height")?,
        natural_width: number("naturalWidth")?,
        natural_height: number("naturalHeight")?,
    };
    (crop.x >= 0.0
        && crop.y >= 0.0
        && crop.width > 0.0
        && crop.height > 0.0
        && crop.natural_width > 0.0
        && crop.natural_height > 0.0)
        .then_some(crop)
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

fn normalize_binding(
    binding_field: SourceField<Option<BindingSource<'_>>>,
    elbowed: bool,
    field: &str,
    policy: &crate::scene_surface::SceneSurfacePolicy,
    context: &mut ElementDiagnosticContext<'_, '_>,
) -> Result<Option<NormalizedBinding>, TryReserveError> {
    let source = match binding_field {
        SourceField::Missing | SourceField::Value(None) => return Ok(None),
        SourceField::Invalid => {
            context.unsupported(
                field,
                &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                Recovery::Substituted,
                None,
            )?;
            return Ok(None);
        }
        SourceField::Value(Some(source)) => source,
    };
    let record_kind = if elbowed {
        "nested.fixed_point_binding"
    } else {
        "nested.point_binding"
    };
    for (position, member) in source.members().iter().enumerate() {
        let semantic_path = pointer_child("", &member.key)?;
        if !crate::scene_surface::is_manifested_path(policy, record_kind, &semantic_path) {
            context.unsupported_property_nested(field, &member.key, position)?;
        }
    }
    let fixed_point = match (elbowed, source.fixed_point()) {
        (true, SourceField::Missing) => return Ok(None),
        (true, SourceField::Value(mut point)) => {
            for component in &mut point {
                if (*component - 0.5).abs() < 1e-4 {
                    *component = 0.5001;
                }
            }
            Some(point)
        }
        (true, SourceField::Invalid) => {
            context.unsupported_nested(
                field,
                "fixedPoint",
                source.member_position("fixedPoint"),
                source.fixed_point_invalid_component(),
                &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                Recovery::Substituted,
            )?;
            Some([0.0, 0.0])
        }
        (false, _) => None,
    };
    let binding_element_id = match source.element_id() {
        SourceField::Value(binding_element_id) if !binding_element_id.is_empty() => {
            binding_element_id
        }
        SourceField::Missing | SourceField::Invalid | SourceField::Value(_) => {
            context.unsupported_nested(
                field,
                "elementId",
                source.member_position("elementId"),
                None,
                &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                Recovery::Omitted,
            )?;
            return Ok(None);
        }
    };
    let gap = match source.gap() {
        SourceField::Value(gap) => gap,
        SourceField::Missing | SourceField::Invalid => {
            context.unsupported_nested(
                field,
                "gap",
                source.member_position("gap"),
                None,
                &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                Recovery::Omitted,
            )?;
            return Ok(None);
        }
    };
    let focus = match source.focus() {
        SourceField::Missing => 0.0,
        SourceField::Value(focus) => focus,
        SourceField::Invalid => {
            context.unsupported_nested(
                field,
                "focus",
                source.member_position("focus"),
                None,
                &[DiagnosticEffect::InputRepair, DiagnosticEffect::Visual],
                Recovery::Omitted,
            )?;
            return Ok(None);
        }
    };
    Ok(Some(NormalizedBinding {
        element_id: try_owned(binding_element_id)?,
        focus: canonical_zero(focus),
        gap: canonical_zero(gap),
        fixed_point,
    }))
}

fn normalized_arrowhead(
    source: SourceField<Option<&str>>,
    default: Option<&str>,
) -> Result<Option<String>, TryReserveError> {
    match source {
        SourceField::Value(Some(value)) if is_arrowhead(value) => Ok(Some(try_owned(value)?)),
        SourceField::Value(None) => Ok(None),
        SourceField::Missing | SourceField::Invalid | SourceField::Value(Some(_)) => {
            default.map(try_owned).transpose()
        }
    }
}

fn arrowhead_is_valid_or_missing(source: SourceField<Option<&str>>) -> bool {
    matches!(source, SourceField::Missing | SourceField::Value(None))
        || matches!(source, SourceField::Value(Some(value)) if is_arrowhead(value))
}

fn is_arrowhead(value: &str) -> bool {
    matches!(
        value,
        "arrow"
            | "bar"
            | "dot"
            | "circle"
            | "circle_outline"
            | "triangle"
            | "triangle_outline"
            | "diamond"
            | "diamond_outline"
            | "crowfoot_one"
            | "crowfoot_many"
            | "crowfoot_one_or_many"
    )
}

fn fractional_indices_are_strictly_ascending(indices: &[Option<&str>]) -> bool {
    indices.iter().all(Option::is_some)
        && indices
            .windows(2)
            .all(|pair| matches!(pair, [Some(left), Some(right)] if left < right))
}

fn is_valid_fractional_key(index: &str) -> bool {
    let bytes = index.as_bytes();
    let Some(integer_length) = bytes.first().and_then(|head| match head {
        b'a'..=b'z' => Some(usize::from(*head - b'a') + 2),
        b'A'..=b'Z' => Some(usize::from(b'Z' - *head) + 2),
        _ => None,
    }) else {
        return false;
    };
    if bytes.len() < integer_length
        || bytes.iter().any(|byte| !FRACTIONAL_DIGITS.contains(byte))
        || bytes[integer_length..].last() == Some(&b'0')
    {
        return false;
    }
    !(bytes[0] == b'A' && bytes.len() == 27 && bytes[1..].iter().all(|byte| *byte == b'0'))
}

fn repair_fractional_indices(source: &[Option<&str>]) -> Result<Vec<String>, TryReserveError> {
    struct InvalidGroup {
        lower: Option<usize>,
        members: Vec<usize>,
        upper: Option<usize>,
    }

    fn valid_between(current: Option<&str>, lower: Option<&str>, upper: Option<&str>) -> bool {
        let Some(current) = current else {
            return false;
        };
        lower.is_none_or(|lower| lower < current) && upper.is_none_or(|upper| current < upper)
    }

    fn lower_bound(
        source: &[Option<&str>],
        previous: Option<usize>,
        index: usize,
    ) -> Option<usize> {
        let previous_value = previous.and_then(|index| source[index]);
        let candidate = index.checked_sub(1).and_then(|index| source[index]);
        if candidate
            .is_some_and(|candidate| previous_value.is_none_or(|previous| candidate > previous))
        {
            index.checked_sub(1)
        } else {
            previous
        }
    }

    fn upper_bound(source: &[Option<&str>], previous: usize, index: usize) -> usize {
        let previous_value = source.get(previous).copied().flatten();
        if previous_value.is_some() && index < previous {
            return previous;
        }
        let mut candidate_index = previous.saturating_add(1);
        while candidate_index < source.len() {
            let candidate = source[candidate_index];
            if candidate
                .is_some_and(|candidate| previous_value.is_none_or(|previous| candidate > previous))
            {
                return candidate_index;
            }
            candidate_index += 1;
        }
        source.len()
    }

    let mut groups = Vec::new();
    let mut lower = None;
    let mut upper = 0;
    let mut index = 0;
    while index < source.len() {
        lower = lower_bound(source, lower, index);
        upper = upper_bound(source, upper, index);
        let lower_value = lower.and_then(|bound| source[bound]);
        let upper_value = source.get(upper).copied().flatten();
        if valid_between(source[index], lower_value, upper_value) {
            index += 1;
            continue;
        }

        let mut members = Vec::new();
        members.try_reserve(source.len().saturating_sub(index))?;
        members.push(index);
        index += 1;
        while index < source.len() {
            let next_lower = lower_bound(source, lower, index);
            let next_upper = upper_bound(source, upper, index);
            if valid_between(
                source[index],
                next_lower.and_then(|bound| source[bound]),
                source.get(next_upper).copied().flatten(),
            ) {
                break;
            }
            lower = next_lower;
            upper = next_upper;
            members.push(index);
            index += 1;
        }
        groups.try_reserve(1)?;
        groups.push(InvalidGroup {
            lower,
            members,
            upper: (upper < source.len()).then_some(upper),
        });
    }

    let mut repaired = Vec::new();
    repaired.try_reserve_exact(source.len())?;
    for index in source {
        repaired.push(match index {
            Some(index) => try_owned(index)?,
            None => String::new(),
        });
    }
    for group in groups {
        let lower = group.lower.map(|index| repaired[index].as_str());
        let upper = group.upper.map(|index| repaired[index].as_str());
        let generated = generate_n_fractional_keys(lower, upper, group.members.len())?;
        for (index, generated) in group.members.into_iter().zip(generated) {
            repaired[index] = generated;
        }
    }
    Ok(repaired)
}

const FRACTIONAL_DIGITS: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn generate_n_fractional_keys(
    lower: Option<&str>,
    upper: Option<&str>,
    count: usize,
) -> Result<Vec<String>, TryReserveError> {
    fn append_keys(
        output: &mut Vec<String>,
        lower: Option<&str>,
        upper: Option<&str>,
        count: usize,
    ) -> Result<(), TryReserveError> {
        if count == 0 {
            return Ok(());
        }
        if count == 1 {
            output.try_reserve(1)?;
            output.push(generate_fractional_key(lower, upper)?);
            return Ok(());
        }
        if upper.is_none() {
            let mut current = generate_fractional_key(lower, None)?;
            output.try_reserve(count)?;
            output.push(try_owned(&current)?);
            for _ in 1..count {
                current = generate_fractional_key(Some(&current), None)?;
                output.push(try_owned(&current)?);
            }
            return Ok(());
        }
        if lower.is_none() {
            let mut reverse = Vec::new();
            reverse.try_reserve_exact(count)?;
            let mut current = generate_fractional_key(None, upper)?;
            reverse.push(try_owned(&current)?);
            for _ in 1..count {
                current = generate_fractional_key(None, Some(&current))?;
                reverse.push(try_owned(&current)?);
            }
            output.try_reserve(count)?;
            output.extend(reverse.into_iter().rev());
            return Ok(());
        }

        let middle = generate_fractional_key(lower, upper)?;
        let left_count = count / 2;
        append_keys(output, lower, Some(&middle), left_count)?;
        output.try_reserve(1)?;
        output.push(try_owned(&middle)?);
        append_keys(output, Some(&middle), upper, count - left_count - 1)
    }

    let mut keys = Vec::new();
    keys.try_reserve_exact(count)?;
    append_keys(&mut keys, lower, upper, count)?;
    Ok(keys)
}

fn generate_fractional_key(
    lower: Option<&str>,
    upper: Option<&str>,
) -> Result<String, TryReserveError> {
    match (lower, upper) {
        (None, None) => try_owned("a0"),
        (None, Some(upper)) => {
            let integer = integer_part(upper);
            let fraction = &upper[integer.len()..];
            if integer == "A00000000000000000000000000" {
                let mut key = try_owned(integer)?;
                let midpoint = fractional_midpoint("", Some(fraction))?;
                try_push_str(&mut key, &midpoint)?;
                return Ok(key);
            }
            if integer < upper {
                return try_owned(integer);
            }
            decrement_fractional_integer(integer)
        }
        (Some(lower), None) => {
            let integer = integer_part(lower);
            let fraction = &lower[integer.len()..];
            if let Some(incremented) = increment_fractional_integer(integer)? {
                Ok(incremented)
            } else {
                let mut key = try_owned(integer)?;
                let midpoint = fractional_midpoint(fraction, None)?;
                try_push_str(&mut key, &midpoint)?;
                Ok(key)
            }
        }
        (Some(lower), Some(upper)) => {
            let lower_integer = integer_part(lower);
            let upper_integer = integer_part(upper);
            if lower_integer == upper_integer {
                let mut key = try_owned(lower_integer)?;
                let midpoint = fractional_midpoint(
                    &lower[lower_integer.len()..],
                    Some(&upper[upper_integer.len()..]),
                )?;
                try_push_str(&mut key, &midpoint)?;
                return Ok(key);
            }
            if let Some(incremented) = increment_fractional_integer(lower_integer)? {
                if incremented.as_str() < upper {
                    return Ok(incremented);
                }
            }
            let mut key = try_owned(lower_integer)?;
            let midpoint = fractional_midpoint(&lower[lower_integer.len()..], None)?;
            try_push_str(&mut key, &midpoint)?;
            Ok(key)
        }
    }
}

fn integer_part(key: &str) -> &str {
    let head = key.as_bytes()[0];
    let length = if head.is_ascii_lowercase() {
        usize::from(head - b'a') + 2
    } else {
        usize::from(b'Z' - head) + 2
    };
    &key[..length]
}

fn increment_fractional_integer(integer: &str) -> Result<Option<String>, TryReserveError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(integer.len().saturating_add(1))?;
    bytes.extend_from_slice(integer.as_bytes());
    let mut carry = true;
    for digit in bytes[1..].iter_mut().rev() {
        if !carry {
            break;
        }
        let position = FRACTIONAL_DIGITS
            .iter()
            .position(|candidate| candidate == digit)
            .unwrap_or(0)
            + 1;
        if position == FRACTIONAL_DIGITS.len() {
            *digit = FRACTIONAL_DIGITS[0];
        } else {
            *digit = FRACTIONAL_DIGITS[position];
            carry = false;
        }
    }
    if carry {
        match bytes[0] {
            b'Z' => return Ok(Some(try_owned("a0")?)),
            b'z' => return Ok(None),
            head => {
                bytes[0] = head + 1;
                if bytes[0] > b'a' {
                    bytes.try_reserve(1)?;
                    bytes.push(b'0');
                } else {
                    bytes.pop();
                }
            }
        }
    }
    let value = std::str::from_utf8(&bytes).expect("fractional keys are ASCII");
    Ok(Some(try_owned(value)?))
}

fn decrement_fractional_integer(integer: &str) -> Result<String, TryReserveError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(integer.len().saturating_add(1))?;
    bytes.extend_from_slice(integer.as_bytes());
    let mut borrow = true;
    for digit in bytes[1..].iter_mut().rev() {
        if !borrow {
            break;
        }
        let position = FRACTIONAL_DIGITS
            .iter()
            .position(|candidate| candidate == digit)
            .unwrap_or(0);
        if position == 0 {
            *digit = *FRACTIONAL_DIGITS
                .last()
                .expect("digit alphabet is nonempty");
        } else {
            *digit = FRACTIONAL_DIGITS[position - 1];
            borrow = false;
        }
    }
    if borrow {
        match bytes[0] {
            b'a' => return try_owned("Zz"),
            head => {
                bytes[0] = head - 1;
                if bytes[0] < b'Z' {
                    bytes.try_reserve(1)?;
                    bytes.push(b'z');
                } else {
                    bytes.pop();
                }
            }
        }
    }
    try_owned(std::str::from_utf8(&bytes).expect("fractional keys are ASCII"))
}

fn fractional_midpoint(lower: &str, upper: Option<&str>) -> Result<String, TryReserveError> {
    let mut output = String::new();
    output.try_reserve(lower.len().max(upper.map_or(0, str::len)).saturating_add(1))?;
    let mut lower = lower.as_bytes();
    let mut upper = upper.map(str::as_bytes);

    if let Some(upper_bytes) = upper {
        let mut common = 0;
        while common < upper_bytes.len()
            && lower.get(common).copied().unwrap_or(b'0') == upper_bytes[common]
        {
            common += 1;
        }
        if common > 0 {
            output.push_str(
                std::str::from_utf8(&upper_bytes[..common]).expect("fractional keys are ASCII"),
            );
            lower = lower.get(common..).unwrap_or_default();
            upper = Some(&upper_bytes[common..]);
        }
    }

    loop {
        let lower_digit = lower
            .first()
            .and_then(|byte| FRACTIONAL_DIGITS.iter().position(|digit| digit == byte))
            .unwrap_or(0);
        let upper_digit = upper
            .and_then(|bytes| bytes.first())
            .and_then(|byte| FRACTIONAL_DIGITS.iter().position(|digit| digit == byte))
            .unwrap_or(FRACTIONAL_DIGITS.len());
        if upper_digit.saturating_sub(lower_digit) > 1 {
            output.push(char::from(
                FRACTIONAL_DIGITS[(lower_digit + upper_digit).div_ceil(2)],
            ));
            return Ok(output);
        }
        if let Some(upper) = upper.filter(|upper| upper.len() > 1) {
            output.push(char::from(upper[0]));
            return Ok(output);
        }
        output.push(char::from(FRACTIONAL_DIGITS[lower_digit]));
        lower = lower.get(1..).unwrap_or_default();
        upper = None;
    }
}

fn normalized_working_bytes(id_bytes: usize, fields: &CommonFields<'_>) -> u64 {
    256 + SCALAR_REFERENCE_FIELD_COUNT * 64
        + owned_field_bytes(id_bytes)
        + owned_field_bytes(fields.stroke_color.len())
        + owned_field_bytes(fields.background_color.len())
        + owned_field_bytes(fields.fill_style.len())
        + owned_field_bytes(fields.stroke_style.len())
}

fn owned_field_bytes(bytes: usize) -> u64 {
    64 + u64::try_from(bytes).unwrap_or(u64::MAX).saturating_add(63) / 64 * 64
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

fn normalized_roundness(
    source: &ElementSource<'_>,
    policy: &'static crate::scene_surface::SceneSurfacePolicy,
) -> Option<NormalizedRoundness> {
    if crate::scene_surface::element_ignores_inherited_path(
        policy,
        source.kind.scene_surface_kind(),
        "/roundness",
    ) {
        return None;
    }
    let SourceField::Value(Some(roundness)) = source.roundness else {
        return None;
    };
    let SourceField::Value(kind @ 1..=3) = roundness.kind() else {
        return None;
    };
    let value = match roundness.value() {
        SourceField::Value(value) if value >= 0.0 => Some(value),
        SourceField::Missing => None,
        SourceField::Value(_) | SourceField::Invalid => return None,
    };
    match kind {
        1 | 2 => Some(NormalizedRoundness::Proportional),
        3 => Some(NormalizedRoundness::Adaptive {
            radius: value.unwrap_or(32.0),
        }),
        _ => None,
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

fn try_push_str(output: &mut String, value: &str) -> Result<(), TryReserveError> {
    output.try_reserve(value.len())?;
    output.push_str(value);
    Ok(())
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
    use super::{normalize_scene, NormalizedElementKind};
    use crate::{
        CompatibilityRevision, DiagnosticCode, DiagnosticEffect, OutputFormat, SceneDocument,
    };

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
    fn fractional_indexes_match_the_pinned_source_order_vectors() {
        for (source, expected) in [
            (vec![None], vec!["a0"]),
            (vec![None, None, None], vec!["a0", "a1", "a2"]),
            (vec![Some("a0"), None, Some("a2")], vec!["a0", "a1", "a2"]),
            (
                vec![Some("a0"), Some("a0"), Some("a2")],
                vec!["a0", "a1", "a2"],
            ),
            (
                vec![Some("a1"), Some("a1"), Some("a2")],
                vec!["a1", "a1V", "a2"],
            ),
            (vec![Some("a2"), Some("a1")], vec!["a2", "a3"]),
            (
                vec![Some("a0"), Some("a2"), Some("a1"), Some("a4")],
                vec!["a0", "a2", "a3", "a4"],
            ),
        ] {
            let repaired = super::repair_fractional_indices(&source)
                .expect("fractional-index repair allocates");
            assert_eq!(repaired, expected);
            let repaired_source = repaired
                .iter()
                .map(|index| Some(index.as_str()))
                .collect::<Vec<_>>();
            assert_eq!(
                super::repair_fractional_indices(&repaired_source)
                    .expect("idempotent fractional-index repair allocates"),
                repaired
            );
        }
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
    fn fractional_key_generation_matches_version_3_2_0_vectors() {
        for (lower, upper, count, expected) in [
            (None, None, 5, vec!["a0", "a1", "a2", "a3", "a4"]),
            (Some("a0"), None, 3, vec!["a1", "a2", "a3"]),
            (None, Some("a2"), 3, vec!["Zz", "a0", "a1"]),
            (Some("a0"), Some("a1"), 3, vec!["a0G", "a0V", "a0l"]),
        ] {
            assert_eq!(
                super::generate_n_fractional_keys(lower, upper, count)
                    .expect("fractional key generation allocates"),
                expected
            );
        }
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
}
