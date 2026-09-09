use crate::color::{self, CanonicalColor};
use crate::font_metadata::FamilyMetrics;
use crate::normalization::{
    NormalizedElement, NormalizedElementKind, NormalizedElementSource, NormalizedText,
};
use crate::outcome::DiagnosticOrder;
use crate::relationships::ResolvedRelationshipGraph;
use crate::resource_accounting::ResourceDelta;
use crate::{
    CompatibilityDiagnostic, DiagnosticCode, DiagnosticEffect, DiagnosticSeverity, ErrorCode,
    ErrorDetails, FiniteValue, Measurement, OutputFormat, Recovery, RenderRequest, ResourceValue,
    SceneDocument, Sizing, ValueKind,
};

use super::bound_text::{
    has_reciprocal_bound_text, resolve_bound_text_masks, resolve_bound_text_placements,
    BoundTextMaskSource,
};
use super::font_demand::{prune_unused_official_fonts, ResolvedFont};
use super::frame_target::{
    relationship_frame_target, resolve_frame_clips, resolve_frame_opacity, EffectiveFrameRendering,
    FrameViewport,
};
use super::geometry::{
    ecmascript_round, image_corner_radius, include_transformed_bounds,
    magic_frame_canvas_outline_stroke_color, resolved_frame_outline, resolved_frame_rect,
    rounded_rect_path, source_layout_bounds, GeneratedGeometry, Path, FRAME_OUTLINE_STROKE_COLOR,
    FRAME_RADIUS,
};
use super::{asset, freedraw, notices, rough, text, try_owned};

#[derive(Debug)]
pub(super) struct ResolvedScene {
    pub(super) logical_width: f64,
    pub(super) logical_height: f64,
    pub(super) include_background: bool,
    pub(super) background_color: CanonicalColor,
    pub(super) fallback_colors: FallbackColors,
    pub(super) elements: Vec<ResolvedElement>,
    pub(super) fallback_font_woff2: Option<Vec<u8>>,
    pub(super) fallback_font_sfnt: Option<Vec<u8>>,
    pub(super) official_fonts: Vec<ResolvedFont>,
    pub(super) mandatory_notices: Option<notices::MandatoryNotices>,
    /// The CJK subset trigger's outcome, shared by glyph selection and the SVG
    /// font demand so they select Xiaolai together or not at all.
    pub(super) cjk_subset_trigger: text::CjkSubsetTrigger,
}

pub(super) fn semantic_identity_hex(
    identity: &[u8; 32],
) -> Result<String, std::collections::TryReserveError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::new();
    encoded.try_reserve_exact(64)?;
    for byte in identity {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

pub(super) fn relationship_asset_authorities(
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

pub(super) fn asset_failure_location(
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

pub(super) fn asset_claim_location(
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

pub(super) fn relationship_element_target(
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

#[derive(Debug)]
pub(super) enum ResolveSceneError {
    Memory,
    InvalidFallbackFont,
    InvalidMandatoryNotice,
    InvalidColor,
    UnboundedGeometry,
}

/// Whether one text element is a live text occurrence, and if so its bound-text
/// container. Empty text and bound text whose container does not bind it back
/// are omitted and contribute nothing, neither glyphs nor font demand.
fn live_text_occurrence(
    text_source: &NormalizedText,
    source_index: usize,
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> Option<Option<usize>> {
    if text_source.text.is_empty() {
        return None;
    }
    let container_source_index = relationship_element_target(
        source_index,
        crate::relationships::RelationshipKind::Mask,
        relationships,
    );
    if container_source_index
        .is_some_and(|container| !has_reciprocal_bound_text(container, source_index, relationships))
    {
        return None;
    }
    Some(container_source_index)
}

/// Evaluates the CJK subset trigger once for the scene, over every live
/// Excalifont occurrence's rendered `text`. That text is the format-neutral
/// carrier of the Excalifont font family scalar set: `originalText` only
/// survives normalization for SVG, and a wrapped `text` keeps every
/// non-whitespace scalar of its `originalText`. The outcome is carried on the
/// resolved scene so glyph selection and the SVG font demand share it, even
/// when local recovery later omits the occurrence that fired it.
fn cjk_subset_trigger(
    outlines: &[(NormalizedElement, Option<GeneratedGeometry>)],
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> text::CjkSubsetTrigger {
    text::CjkSubsetTrigger::evaluate(outlines.iter().filter_map(|(source, _)| {
        let NormalizedElementKind::Text(text_source) = &source.kind else {
            return None;
        };
        live_text_occurrence(text_source, source.source.source_index(), relationships)?;
        (text_source.font_family == text::EXCALIFONT_FONT_FAMILY)
            .then_some(text_source.text.as_str())
    }))
}

pub(super) fn format_error_code(format: OutputFormat) -> ErrorCode {
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

/// The scene-level failure a label, frame name, official font or fallback font
/// error becomes. `mandatory_notices_json` is the one caller that maps
/// `InvalidFont` elsewhere, to `InvalidMandatoryNotice`, so this stays a named
/// function rather than a `From` impl `?` would apply there too.
fn label_error(error: text::LabelError) -> ResolveSceneError {
    match error {
        text::LabelError::Memory => ResolveSceneError::Memory,
        text::LabelError::InvalidFont => ResolveSceneError::InvalidFallbackFont,
    }
}

/// What every element's resolution reads once the scene's bounds are settled:
/// the request, the relationship graph, the effective frame rendering, the
/// semantic frames that clip and fade their children, the bound-text masks,
/// the canvas background, the CJK subset trigger's outcome, and the layout
/// origin every canvas placement subtracts. Per-element state — the frame
/// clips and opacity the loop computes first, and the official fonts the text
/// and label resolutions load — travels beside it.
struct ResolveContext<'a> {
    request: &'a RenderRequest,
    relationships: &'a ResolvedRelationshipGraph,
    frame_rendering: EffectiveFrameRendering,
    semantic_frames: &'a [SemanticFrameSource],
    bound_text_masks: &'a [BoundTextMaskSource],
    background_color: CanonicalColor,
    cjk_subset_trigger: text::CjkSubsetTrigger,
    min_x: f64,
    min_y: f64,
}

impl ResolveContext<'_> {
    /// Canvas placement of a source box whose top-left corner is `(x, y)`: the
    /// scene origin moves to the padding, and the rotation is about the given
    /// center in the box's own space.
    fn transform(
        &self,
        x: f64,
        y: f64,
        angle_radians: f64,
        center_x: f64,
        center_y: f64,
    ) -> ResolvedTransform {
        ResolvedTransform {
            translation_x: x - self.min_x + self.request.padding(),
            translation_y: y - self.min_y + self.request.padding(),
            angle_radians,
            center_x,
            center_y,
        }
    }

    /// The placement of an element rotated about the center of its own box.
    fn box_transform(&self, source: &NormalizedElement) -> ResolvedTransform {
        self.transform(
            source.x,
            source.y,
            source.angle,
            source.width / 2.0,
            source.height / 2.0,
        )
    }

    /// The clips the frame that owns `source_index` applies to it.
    fn frame_clips(&self, source_index: usize) -> Result<Vec<ResolvedClip>, ResolveSceneError> {
        resolve_frame_clips(
            source_index,
            self.semantic_frames,
            self.relationships,
            self.frame_rendering,
            self.min_x,
            self.min_y,
            self.request.padding(),
        )
    }

    /// The opacity the frame that owns `source_index` multiplies into it.
    fn frame_opacity(&self, source_index: usize) -> f64 {
        resolve_frame_opacity(
            source_index,
            self.semantic_frames,
            self.relationships,
            self.frame_rendering,
        )
    }
}

impl ResolvedScene {
    pub(super) fn empty(
        background_color: &str,
        request: &RenderRequest,
        viewport: Option<FrameViewport>,
    ) -> Result<Self, ResolveSceneError> {
        let (logical_width, logical_height) = viewport.map_or_else(
            || {
                let logical_size = request.padding() * 2.0;
                (logical_size.max(1.0), logical_size.max(1.0))
            },
            |viewport| {
                (
                    viewport.max_x - viewport.min_x + request.padding() * 2.0,
                    viewport.max_y - viewport.min_y + request.padding() * 2.0,
                )
            },
        );
        if !logical_width.is_finite() || !logical_height.is_finite() {
            return Err(ResolveSceneError::UnboundedGeometry);
        }
        Ok(Self {
            logical_width: logical_width.max(1.0),
            logical_height: logical_height.max(1.0),
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
            cjk_subset_trigger: text::CjkSubsetTrigger::default(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn elements(
        mut outlines: Vec<(NormalizedElement, Option<GeneratedGeometry>)>,
        images: ResolvedImageInputs,
        fallbacks: Vec<crate::scene_surface::SurfaceFallback>,
        relationships: &crate::relationships::ResolvedRelationshipGraph,
        frame_rendering: EffectiveFrameRendering,
        background_color: &str,
        request: &RenderRequest,
        viewport: Option<FrameViewport>,
    ) -> Result<Option<Self>, ResolveSceneError> {
        if outlines.is_empty() && fallbacks.is_empty() {
            return Ok(None);
        }

        let text_placements = resolve_bound_text_placements(&outlines, relationships)?;
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
        let bound_text_masks = resolve_bound_text_masks(&outlines, relationships)?;

        // The reference synthesizes every frame's name as a text element before
        // it measures the scene, so the names take part in the bounds and are
        // shaped before them.
        let shows_frame_names =
            frame_rendering.enabled && frame_rendering.name && frame_rendering.emit_frame_elements;
        let has_frames = outlines.iter().any(|(source, _)| {
            matches!(
                source.kind,
                NormalizedElementKind::Frame(_) | NormalizedElementKind::MagicFrame(_)
            )
        });
        let fallback_font = if (shows_frame_names && has_frames)
            || fallbacks.iter().any(|fallback| fallback.paint)
        {
            Some(text::FallbackFont::load().map_err(label_error)?)
        } else {
            None
        };
        let mut frame_names = Vec::<(usize, text::FrameName)>::new();
        if shows_frame_names {
            for (source, _) in &outlines {
                let (NormalizedElementKind::Frame(frame)
                | NormalizedElementKind::MagicFrame(frame)) = &source.kind
                else {
                    continue;
                };
                let title = frame.name.as_deref().unwrap_or({
                    if matches!(source.kind, NormalizedElementKind::MagicFrame(_)) {
                        "AI Frame"
                    } else {
                        "Frame"
                    }
                });
                let font = fallback_font
                    .as_ref()
                    .ok_or(ResolveSceneError::InvalidFallbackFont)?;
                let name = font
                    .resolve_frame_name(title, source.width)
                    .map_err(label_error)?;
                frame_names.try_reserve(1)?;
                frame_names.push((source.source.source_index(), name));
            }
        }

        let (mut min_x, mut min_y, mut max_x, mut max_y) = viewport.map_or(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |viewport| {
                (
                    viewport.min_x,
                    viewport.min_y,
                    viewport.max_x,
                    viewport.max_y,
                )
            },
        );
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
            if let Some((_, name)) = frame_names
                .iter()
                .find(|(index, _)| *index == source.source.source_index())
            {
                // The name is an unrotated text element at the frame's x, its
                // line box `nameOffsetY` above the frame's top edge. It belongs
                // to no frame, so it extends the bounds even when the frame it
                // names is itself a clipped child of another frame.
                include_transformed_bounds(
                    source.x,
                    source.y - text::FRAME_NAME_OFFSET_Y - text::FRAME_NAME_BOX_HEIGHT,
                    name.width,
                    text::FRAME_NAME_BOX_HEIGHT,
                    0.0,
                    &mut min_x,
                    &mut min_y,
                    &mut max_x,
                    &mut max_y,
                );
            }
            if frame_rendering.enabled
                && frame_rendering.clip
                && relationship_frame_target(source.source.source_index(), relationships).is_some()
            {
                continue;
            }
            if viewport.is_some() {
                continue;
            }
            let (local_min_x, local_min_y, local_max_x, local_max_y) =
                geometry.as_ref().map_or_else(
                    || source_layout_bounds(source),
                    |geometry| geometry.bounds(source),
                );
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
            if viewport.is_some() {
                continue;
            }
            let is_clipped_child = frame_rendering.enabled
                && frame_rendering.clip
                && fallback.element_id.as_deref().is_some_and(|id| {
                    relationships
                        .nodes
                        .iter()
                        .find(|node| node.id == id)
                        .and_then(|node| {
                            relationship_frame_target(node.source_index, relationships)
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
            return Self::empty(background_color, request, viewport).map(Some);
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
                    source_index: fallback.source_index,
                    x: fallback.x,
                    y: fallback.y,
                    width: fallback.width,
                    height: fallback.height,
                    angle: fallback.angle,
                });
            }
        }
        let mut official_fonts = Vec::<text::OfficialFont>::new();
        let cjk_subset_trigger = cjk_subset_trigger(&outlines, relationships);
        let mut has_label = false;
        let frame_name_color = match request.color_mode() {
            crate::ColorMode::Light => FRAME_NAME_COLOR_LIGHT,
            crate::ColorMode::Dark => FRAME_NAME_COLOR_DARK,
        }
        .for_mode(request.color_mode());
        let context = ResolveContext {
            request,
            relationships,
            frame_rendering,
            semantic_frames: &semantic_frames,
            bound_text_masks: &bound_text_masks,
            background_color,
            cjk_subset_trigger,
            min_x,
            min_y,
        };
        for (source, geometry) in outlines {
            let frame_clips = context.frame_clips(source.source.source_index())?;
            let frame_opacity = context.frame_opacity(source.source.source_index());
            if matches!(
                source.kind,
                NormalizedElementKind::Frame(_) | NormalizedElementKind::MagicFrame(_)
            ) {
                if frame_rendering.enabled && frame_rendering.emit_frame_elements {
                    let name = frame_names
                        .iter()
                        .position(|(index, _)| *index == source.source.source_index())
                        .map(|position| frame_names.swap_remove(position).1)
                        .map(|name| ResolvedFrameName {
                            transform: context.transform(
                                source.x,
                                source.y - text::FRAME_NAME_OFFSET_Y - text::FRAME_NAME_BOX_HEIGHT,
                                0.0,
                                name.width / 2.0,
                                text::FRAME_NAME_BOX_HEIGHT / 2.0,
                            ),
                            color: frame_name_color,
                            label: name.label,
                        });
                    has_label |= name.is_some();
                    let outline = frame_rendering
                        .outline
                        .then(|| {
                            ResolvedFrameOutline::new(
                                source.width,
                                source.height,
                                matches!(source.kind, NormalizedElementKind::MagicFrame(_)),
                                request.color_mode(),
                            )
                        })
                        .transpose()?;
                    let transform = context.box_transform(&source);
                    elements.push(ResolvedElement::Frame(ResolvedFrame {
                        source_order: source.source_order,
                        source: source.source,
                        name,
                        outline,
                        frame_clips,
                        transform,
                        opacity: source.opacity / 100.0 * frame_opacity,
                        link: source.link,
                        width: source.width,
                        height: source.height,
                    }));
                }
                continue;
            }
            if let NormalizedElementKind::Text(text_source) = &source.kind {
                let Some(container_source_index) =
                    live_text_occurrence(text_source, source.source.source_index(), relationships)
                else {
                    continue;
                };
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
                            .map_err(label_error)?,
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
                            font_family: text_source.font_family,
                        },
                    )
                    .map_err(label_error)?;
                laid_out.font_index = font_index;
                if !laid_out.missing_codepoints.is_empty()
                    || text::contains_bidirectional_text(&text_source.text)
                {
                    laid_out = text::layout_with_fallbacks(
                        font_index,
                        &mut official_fonts,
                        &text_source.text,
                        text::TextLayout {
                            width: source.width,
                            font_size: text_source.font_size,
                            line_height: text_source.line_height,
                            text_align: &text_source.text_align,
                            font_family: text_source.font_family,
                        },
                        context.cjk_subset_trigger,
                    )
                    .map_err(label_error)?
                    .0;
                }
                let transform = context.box_transform(&source);
                elements.push(ResolvedElement::Text(ResolvedTextElement {
                    source_order: source.source_order,
                    source: source.source,
                    font_family: text_source.font_family,
                    original_text: try_owned(&text_source.original_text)?,
                    text: laid_out,
                    frame_clips,
                    transform,
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
                    let transform = context.box_transform(&source);
                    elements.push(ResolvedElement::Image(ResolvedImage {
                        source_order: source.source_order,
                        source: source.source,
                        dark_mode_filter: DarkModeImageFilter::for_image(
                            request.color_mode(),
                            asset,
                        ),
                        asset: std::sync::Arc::clone(asset),
                        frame_clips,
                        corner_clip: ResolvedImageCornerClip::for_image(
                            source.width,
                            source.height,
                            source.roundness,
                        )?,
                        transform,
                        opacity: source.opacity / 100.0 * frame_opacity,
                        source_rect,
                        projection: ResolvedImageProjection::for_image(
                            source.width,
                            source.height,
                            image.crop,
                        ),
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
                    let transform = context.box_transform(&source);
                    elements.push(ResolvedElement::ImageFallback(ResolvedImageFallback {
                        source_order: source.source_order,
                        source: source.source,
                        status: *status,
                        frame_clips,
                        transform,
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
            let transform = context.transform(
                source.x,
                source.y,
                source.angle,
                (local_min_x + local_max_x) / 2.0,
                (local_min_y + local_max_y) / 2.0,
            );
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
                                .position(|font| font.matches_family(EMBEDDABLE_LABEL_FONT_FAMILY))
                            {
                                index
                            } else {
                                official_fonts.try_reserve(1)?;
                                official_fonts.push(
                                    text::OfficialFont::load(EMBEDDABLE_LABEL_FONT_FAMILY, label)
                                        .map_err(label_error)?,
                                );
                                official_fonts.len() - 1
                            };
                            let wrapped_label = official_fonts[font_index]
                                .wrap_embeddable_label(label, font_size, source.width - 20.0)
                                .map_err(label_error)?;
                            let mut laid_out = official_fonts[font_index]
                                .layout(
                                    &wrapped_label,
                                    text::TextLayout {
                                        width: source.width,
                                        font_size,
                                        line_height: EMBEDDABLE_LABEL_LINE_HEIGHT,
                                        text_align: "center",
                                        font_family: EMBEDDABLE_LABEL_FONT_FAMILY,
                                    },
                                )
                                .map_err(label_error)?;
                            laid_out.font_index = font_index;
                            if !laid_out.missing_codepoints.is_empty()
                                || text::contains_bidirectional_text(&wrapped_label)
                            {
                                laid_out = text::layout_with_fallbacks(
                                    font_index,
                                    &mut official_fonts,
                                    &wrapped_label,
                                    text::TextLayout {
                                        width: source.width,
                                        font_size,
                                        line_height: EMBEDDABLE_LABEL_LINE_HEIGHT,
                                        text_align: "center",
                                        font_family: EMBEDDABLE_LABEL_FONT_FAMILY,
                                    },
                                    context.cjk_subset_trigger,
                                )
                                .map_err(label_error)?
                                .0;
                            }
                            let line_count = laid_out.lines.len() as f64;
                            let offset_y = (source.height
                                - line_count * font_size * EMBEDDABLE_LABEL_LINE_HEIGHT)
                                / 2.0;
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
                        background_color: context.background_color,
                        label_mask: context
                            .bound_text_masks
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
                .map_err(label_error)?;
            has_label |= label.is_some();
            let source_index = fallback.element_id.as_deref().and_then(|id| {
                relationships
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .map(|node| node.source_index)
            });
            let frame_clips = if let Some(source_index) = source_index {
                context.frame_clips(source_index)?
            } else {
                let frame = fallback.frame_id.as_deref().and_then(|frame_id| {
                    fallback_frame_clips
                        .iter()
                        .find(|frame| frame.id == frame_id)
                });
                let mut clips = Vec::new();
                if let Some(frame) = frame {
                    clips.try_reserve_exact(1)?;
                    clips.push(ResolvedClip::for_frame(
                        frame.source_index,
                        frame.width,
                        frame.height,
                        true,
                        context.transform(
                            frame.x,
                            frame.y,
                            frame.angle,
                            frame.width / 2.0,
                            frame.height / 2.0,
                        ),
                    )?);
                }
                clips
            };
            let frame_opacity =
                source_index.map_or(1.0, |source_index| context.frame_opacity(source_index));
            let transform = context.transform(
                fallback.x,
                fallback.y,
                fallback.angle,
                fallback.width / 2.0,
                fallback.height / 2.0,
            );
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
                transform,
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
        prune_unused_official_fonts(&mut elements, &mut official_fonts)?;
        let has_official_text = elements.iter().any(|element| match element {
            ResolvedElement::Text(_) => true,
            ResolvedElement::Outline(outline) => outline.generated_label.is_some(),
            _ => false,
        });
        let mut resolved_fonts = Vec::new();
        resolved_fonts.try_reserve_exact(official_fonts.len())?;
        for font in official_fonts {
            let (family, pdf_name, identity, svg_mime, woff2, sfnt) = font.into_parts();
            resolved_fonts.push(ResolvedFont {
                family,
                pdf_name,
                identity,
                svg_mime,
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
            cjk_subset_trigger,
        }))
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
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

    pub(super) fn raster_or_pdf_scale(&self, sizing: Sizing) -> f64 {
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

#[derive(Debug)]
pub(super) enum ResolvedElement {
    Outline(ResolvedOutline),
    Freedraw(ResolvedFreedraw),
    Frame(ResolvedFrame),
    Text(ResolvedTextElement),
    Image(ResolvedImage),
    ImageFallback(ResolvedImageFallback),
    Fallback(ResolvedFallback),
}

impl ResolvedElement {
    pub(super) fn source_order(&self) -> u64 {
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

    pub(super) fn paint_order(&self) -> (bool, u64, u8) {
        match self {
            Self::Text(text) => text
                .container_source_order
                .map_or((false, text.source_order, 0), |order| (false, order, 1)),
            Self::Outline(outline) => (outline.final_pass, outline.source_order, 0),
            _ => (false, self.source_order(), 0),
        }
    }
}

#[derive(Debug)]
pub(super) struct ResolvedImageAsset {
    pub(super) identity: [u8; 32],
    pub(super) source_bytes: std::sync::Arc<Vec<u8>>,
    pub(super) media_type: &'static str,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba: Option<std::sync::Arc<Vec<u8>>>,
    pub(super) premultiplied_rgba: Option<std::sync::Arc<Vec<u8>>>,
    pub(super) sanitized_svg: Option<std::sync::Arc<Vec<u8>>>,
    pub(super) svg_effects: Vec<asset::svg::Effect>,
    pub(super) svg_fallback_pointer: Option<String>,
}

#[derive(Debug)]
pub(super) struct ResolvedImageInputs {
    pub(super) assets: std::collections::BTreeMap<usize, std::sync::Arc<ResolvedImageAsset>>,
    pub(super) fallbacks: Vec<(usize, crate::normalization::NormalizedImageStatus)>,
}

#[derive(Debug)]
pub(super) struct ResolvedImage {
    source_order: u64,
    pub(super) source: NormalizedElementSource,
    pub(super) asset: std::sync::Arc<ResolvedImageAsset>,
    /// The reference's dark-mode filter chain this image passes through, or
    /// `None` in light mode.
    pub(super) dark_mode_filter: Option<DarkModeImageFilter>,
    pub(super) frame_clips: Vec<ResolvedClip>,
    /// The rounded-corner clip of an image with a roundness record, in the
    /// element's own space before crop, flip, and rotation; `None` without one.
    pub(super) corner_clip: Option<ResolvedImageCornerClip>,
    pub(super) transform: ResolvedTransform,
    pub(super) opacity: f64,
    /// The crop in the asset's pixel grid, the canvas `drawImage` source
    /// rectangle PNG draws from.
    pub(super) source_rect: ResolvedImageSourceRect,
    /// The reference SVG export's viewport and content placement, which SVG
    /// and PDF draw through.
    pub(super) projection: ResolvedImageProjection,
    pub(super) scale: [i8; 2],
    pub(super) link: Option<String>,
    /// The element's unrounded size: its flip axis, rotation center, corner
    /// clip, link rectangle, and the canvas rectangle PNG draws into.
    pub(super) width: f64,
    pub(super) height: f64,
}

/// The reference export's dark-mode filters an image passes through, resolved
/// once from the color mode and the asset kind so each format lowers the chain
/// its authority applies (spec section 8.2). The reference SVG puts
/// `IMAGE_INVERT_FILTER` on a raster image's `<use>` under its root
/// `THEME_FILTER` and only the root filter on a nested-SVG image; the reference
/// canvas export resets its filter for a raster image and keeps the theme
/// filter for a nested-SVG image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DarkModeImageFilter {
    /// `IMAGE_INVERT_FILTER` followed by `THEME_FILTER`. SVG references it and
    /// PDF, whose authority is official SVG, lowers it into the pixels; PNG
    /// follows the canvas and leaves a raster image's pixels alone.
    RasterImage,
    /// `THEME_FILTER` alone. SVG references it and PNG lowers it into the
    /// pixels it rasterizes; PDF embeds the nested SVG's vector content
    /// unlowered until #154 decides the mechanism.
    NestedSvgImage,
}

impl DarkModeImageFilter {
    fn for_image(mode: crate::ColorMode, asset: &ResolvedImageAsset) -> Option<Self> {
        match mode {
            crate::ColorMode::Light => None,
            crate::ColorMode::Dark if asset.sanitized_svg.is_some() => Some(Self::NestedSvgImage),
            crate::ColorMode::Dark => Some(Self::RasterImage),
        }
    }
}

/// The clip the reference applies to an image element with a `roundness`
/// record: its rectangle with `getCornerRadius(min(width, height))` corners,
/// which its SVG defines as a `<clipPath>` holding a `<rect rx ry>` and its
/// canvas paints through `roundRect`. Like a frame clip it carries both the
/// radius for the SVG rectangle and the same outline as a path for the
/// backends that rasterize or emit one.
#[derive(Debug)]
pub(super) struct ResolvedImageCornerClip {
    pub(super) radius: f64,
    pub(super) path: Path,
}

impl ResolvedImageCornerClip {
    /// The corner clip of an image element, or `None` when it has no roundness
    /// record or its radius is not positive: a zero radius clips nothing the
    /// image's rectangle does not already clip.
    pub(super) fn for_image(
        width: f64,
        height: f64,
        roundness: Option<crate::normalization::NormalizedRoundness>,
    ) -> Result<Option<Self>, ResolveSceneError> {
        let radius = image_corner_radius(width, height, roundness);
        if !radius.is_finite() || radius <= 0.0 {
            return Ok(None);
        }
        Ok(Some(Self {
            radius,
            path: rounded_rect_path(width, height, radius)?,
        }))
    }
}

#[derive(Debug)]
pub(super) struct ResolvedImageFallback {
    source_order: u64,
    pub(super) source: NormalizedElementSource,
    pub(super) status: crate::normalization::NormalizedImageStatus,
    pub(super) frame_clips: Vec<ResolvedClip>,
    pub(super) transform: ResolvedTransform,
    pub(super) opacity: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

/// How the reference SVG export places an image, which SVG and PDF (whose
/// visual authority is the official SVG, spec section 8.6) reproduce: the
/// whole asset is drawn into the content rectangle and clipped to the
/// viewport, both in the element's own space before its `scale` flip about the
/// element's unrounded center and its rotation.
///
/// The viewport is `Math.round(width)` by `Math.round(height)`, the size of
/// the reference's `<use>` and of its crop `<mask>`; it is the only image value
/// that rounds (spec section 8.6). An uncropped image fills the rounded
/// viewport, as the reference's `<symbol>` image of `width="100%"` does; a
/// cropped image keeps the reference's unrounded `getUncroppedWidthAndHeight`
/// size at the negative `normalizedCrop` offset of its group translation, and
/// the viewport clips it.
///
/// PNG follows the canvas, which draws the crop into the element's unrounded
/// rectangle (`drawImage(..., 0, 0, element.width, element.height)`), and
/// does not use this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ResolvedImageProjection {
    pub(super) viewport_width: f64,
    pub(super) viewport_height: f64,
    /// A cropped image's placement; `None` for an uncropped image, whose
    /// content is the viewport itself.
    pub(super) crop: Option<ResolvedImageCropPlacement>,
}

/// A cropped image's placement in the reference export: the whole image at
/// its unrounded `getUncroppedWidthAndHeight` size, shifted up and left by the
/// `normalizedCrop` offset so the crop's top-left corner sits at the element's
/// origin. Neither value inherits the viewport's rounding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ResolvedImageCropPlacement {
    pub(super) offset_x: f64,
    pub(super) offset_y: f64,
    pub(super) uncropped_width: f64,
    pub(super) uncropped_height: f64,
}

impl ResolvedImageProjection {
    /// The projection of an image element of the given unrounded size and
    /// normalized crop record, if it has one.
    pub(super) fn for_image(
        width: f64,
        height: f64,
        crop: Option<crate::normalization::NormalizedImageCrop>,
    ) -> Self {
        Self {
            viewport_width: ecmascript_round(width),
            viewport_height: ecmascript_round(height),
            crop: crop.map(|crop| {
                // The reference's arithmetic, operation for operation, so the
                // emitted numbers are its numbers: `getUncroppedWidthAndHeight`
                // divides the element size by the crop's share of the natural
                // size, and the normalized crop offset divides the crop offset
                // by the natural-to-uncropped ratio.
                let uncropped_width = width / (crop.width / crop.natural_width);
                let uncropped_height = height / (crop.height / crop.natural_height);
                ResolvedImageCropPlacement {
                    offset_x: crop.x / (crop.natural_width / uncropped_width),
                    offset_y: crop.y / (crop.natural_height / uncropped_height),
                    uncropped_width,
                    uncropped_height,
                }
            }),
        }
    }

    /// The rectangle the whole asset is drawn into, as `(x, y, width, height)`
    /// in the element's space before its flip.
    pub(super) fn content_rect(&self) -> (f64, f64, f64, f64) {
        match self.crop {
            None => (0.0, 0.0, self.viewport_width, self.viewport_height),
            Some(crop) => (
                -crop.offset_x,
                -crop.offset_y,
                crop.uncropped_width,
                crop.uncropped_height,
            ),
        }
    }

    /// The reference `<use>` viewport, as `(x, y, width, height)` in the
    /// element's space before its flip: from the content's origin to the
    /// rounded viewport's far edges, so `width + normalizedCropX` wide as the
    /// reference sizes it, and the viewport itself for an uncropped image.
    pub(super) fn viewport_rect(&self) -> (f64, f64, f64, f64) {
        let (content_x, content_y, _, _) = self.content_rect();
        (
            content_x,
            content_y,
            self.viewport_width - content_x,
            self.viewport_height - content_y,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ResolvedImageSourceRect {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Debug)]
pub(super) struct ResolvedTextElement {
    pub(super) source_order: u64,
    pub(super) source: NormalizedElementSource,
    pub(super) font_family: i32,
    pub(super) original_text: String,
    pub(super) text: text::ResolvedText,
    pub(super) frame_clips: Vec<ResolvedClip>,
    pub(super) transform: ResolvedTransform,
    pub(super) color: CanonicalColor,
    pub(super) opacity: f64,
    pub(super) render: bool,
    pub(super) base_element_work_units: u64,
    container_source_order: Option<u64>,
    pub(super) link: Option<String>,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Debug)]
pub(super) struct ResolvedOutline {
    pub(super) source_order: u64,
    pub(super) source: NormalizedElementSource,
    pub(super) base_element_work_units: u64,
    pub(super) drawing: rough::Drawing,
    pub(super) frame_clips: Vec<ResolvedClip>,
    pub(super) transform: ResolvedTransform,
    pub(super) stroke_color: CanonicalColor,
    pub(super) fill_color: Option<CanonicalColor>,
    pub(super) stroke_width: f64,
    pub(super) stroke_style: StrokeStyle,
    pub(super) source_stroke_width: f64,
    pub(super) opacity: f64,
    pub(super) element_opacity: f64,
    pub(super) background_color: CanonicalColor,
    pub(super) label_mask: Option<ResolvedLabelMask>,
    pub(super) link: Option<String>,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) generated_label: Option<text::ResolvedText>,
    pub(super) generated_label_color: Option<CanonicalColor>,
    pub(super) generated_label_offset_y: f64,
    final_pass: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ResolvedLabelMask {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Debug)]
pub(super) struct ResolvedFreedraw {
    source_order: u64,
    pub(super) source: NormalizedElementSource,
    pub(super) base_element_work_units: u64,
    pub(super) path: freedraw::Path,
    pub(super) background_fill: Option<rough::Fill>,
    pub(super) frame_clips: Vec<ResolvedClip>,
    pub(super) transform: ResolvedTransform,
    pub(super) color: CanonicalColor,
    pub(super) background_color: Option<CanonicalColor>,
    pub(super) source_stroke_width: f64,
    pub(super) opacity: f64,
    pub(super) link: Option<String>,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Debug)]
pub(super) struct ResolvedFrame {
    pub(super) source_order: u64,
    pub(super) source: NormalizedElementSource,
    pub(super) name: Option<ResolvedFrameName>,
    pub(super) outline: Option<ResolvedFrameOutline>,
    pub(super) frame_clips: Vec<ResolvedClip>,
    pub(super) transform: ResolvedTransform,
    pub(super) opacity: f64,
    pub(super) link: Option<String>,
    pub(super) width: f64,
    pub(super) height: f64,
}

/// A frame's outline in a scene export: the frame rectangle with
/// `FRAME_STYLE.radius` corners, stroked `FRAME_STYLE.strokeWidth` wide in the
/// reference's outline colors, lowered into the color mode like every other
/// color the reference paints under its dark-mode filter.
#[derive(Debug)]
pub(super) struct ResolvedFrameOutline {
    pub(super) path: Path,
    /// `FRAME_STYLE.strokeColor` for the mode: the stroke the reference's SVG
    /// export gives every frame kind, which SVG and PDF paint.
    pub(super) stroke_color: CanonicalColor,
    /// The stroke the reference's canvas export gives the frame for the mode,
    /// which PNG paints: `FRAME_STYLE.strokeColor` for an ordinary frame and
    /// the theme's magic-frame color for a magic frame.
    pub(super) canvas_stroke_color: CanonicalColor,
}

impl ResolvedFrameOutline {
    fn new(
        width: f64,
        height: f64,
        magic: bool,
        mode: crate::ColorMode,
    ) -> Result<Self, ResolveSceneError> {
        let stroke_color = FRAME_OUTLINE_STROKE_COLOR.for_mode(mode);
        Ok(Self {
            path: resolved_frame_outline(width, height)?,
            stroke_color,
            canvas_stroke_color: if magic {
                magic_frame_canvas_outline_stroke_color(mode)
            } else {
                stroke_color
            },
        })
    }
}

/// A frame's visible name as the reference export renders it: a synthesized
/// text element (`addFrameLabelsAsTextElements`) rather than a decoration of
/// the frame. It has its own unrotated placement above the frame's top-left
/// corner, full opacity, no link, and no frame clip, whatever the frame has.
#[derive(Debug)]
pub(super) struct ResolvedFrameName {
    pub(super) label: text::ResolvedLabel,
    /// Canvas placement of the label box's top-left corner; the angle is 0.
    pub(super) transform: ResolvedTransform,
    /// `FRAME_STYLE.nameColorLightTheme` or `nameColorDarkTheme` for the mode.
    pub(super) color: CanonicalColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StrokeStyle {
    Solid,
    Dashed,
    Dotted,
}

pub(super) fn decoration_fill_color(
    decoration: &rough::Decoration,
    stroke: CanonicalColor,
    background: CanonicalColor,
) -> CanonicalColor {
    match decoration.fill_paint {
        rough::DecorationFillPaint::Stroke => stroke,
        rough::DecorationFillPaint::Background => background,
    }
}

pub(super) fn decoration_stroke_style(
    decoration: &rough::Decoration,
    source_stroke_width: f64,
) -> (StrokeStyle, f64) {
    if decoration.dotted {
        (StrokeStyle::Dotted, source_stroke_width - 2.0)
    } else {
        (StrokeStyle::Solid, source_stroke_width)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FallbackColors {
    pub(super) fill: CanonicalColor,
    pub(super) stroke: CanonicalColor,
    pub(super) label: CanonicalColor,
}

/// Excalidraw creates the offline placeholder label as Helvetica text, so the
/// label is shaped with the Liberation Sans substitute and positioned by
/// Helvetica's metadata metrics.
const EMBEDDABLE_LABEL_FONT_FAMILY: i32 = 2;

/// `FRAME_STYLE.nameColorLightTheme` and `nameColorDarkTheme`: the frame name
/// is a text element whose stroke color the export picks by theme, and the
/// dark one then passes through the dark-mode filter like every other color.
const FRAME_NAME_COLOR_LIGHT: CanonicalColor = CanonicalColor::from_rgb8(0x99, 0x99, 0x99);
const FRAME_NAME_COLOR_DARK: CanonicalColor = CanonicalColor::from_rgb8(0x7a, 0x7a, 0x7a);

/// The label is a new text element with no stored height, so its line height
/// is its family's metadata line height (`getLineHeight`), 1.15 for Helvetica.
const EMBEDDABLE_LABEL_LINE_HEIGHT: f64 =
    match FamilyMetrics::for_source_family(EMBEDDABLE_LABEL_FONT_FAMILY) {
        Some(metrics) => metrics.line_height,
        None => panic!("the embeddable label family has metadata metrics"),
    };

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
    pub(super) fn for_mode(mode: crate::ColorMode) -> Self {
        Self {
            fill: CanonicalColor::from_rgb8(0xf1, 0xf3, 0xf5).for_mode(mode),
            stroke: CanonicalColor::from_rgb8(0x86, 0x8e, 0x96).for_mode(mode),
            label: CanonicalColor::from_rgb8(0x49, 0x50, 0x57).for_mode(mode),
        }
    }
}

#[derive(Debug)]
pub(super) struct ResolvedFallback {
    pub(super) source_order: u64,
    pub(super) source: FallbackSource,
    pub(super) label: Option<text::ResolvedLabel>,
    pub(super) link: Option<String>,
    pub(super) frame_clips: Vec<ResolvedClip>,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) transform: ResolvedTransform,
    pub(super) opacity: f64,
    pub(super) render: bool,
}

struct FallbackFrameClipSource {
    id: String,
    source_index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
}

#[derive(Debug)]
pub(super) struct SemanticFrameSource {
    pub(super) source_index: usize,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) angle: f64,
    pub(super) opacity: f64,
}

#[derive(Debug)]
pub(super) struct ResolvedClip {
    /// Source index of the frame whose outline this clip reproduces. Every
    /// element clipped by the same frame shares one clip identity, which lets
    /// the PNG encoder build and charge that frame's alpha mask once and the
    /// SVG define it once.
    pub(super) frame_source_index: usize,
    /// The clip outline as a path, for the backends that rasterize or emit it
    /// as one.
    pub(super) path: Path,
    /// The same outline as the frame rectangle the reference SVG defines its
    /// `<clipPath>` with: the frame's size and `FRAME_STYLE.radius` for a scene
    /// export, or no radius for the rectangular frame-export clip.
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) radius: f64,
    pub(super) transform: ResolvedTransform,
}

impl ResolvedClip {
    /// The clip one frame applies to its children: the frame's rectangle with
    /// `FRAME_STYLE.radius` corners in a scene export, or the reference's
    /// rectangular target clip in a frame export.
    pub(super) fn for_frame(
        frame_source_index: usize,
        width: f64,
        height: f64,
        rounded: bool,
        transform: ResolvedTransform,
    ) -> Result<Self, ResolveSceneError> {
        let (path, radius) = if rounded {
            (resolved_frame_outline(width, height)?, FRAME_RADIUS)
        } else {
            (resolved_frame_rect(width, height)?, 0.0)
        };
        Ok(Self {
            frame_source_index,
            path,
            width,
            height,
            radius,
            transform,
        })
    }
}

impl ResolvedFallback {
    pub(super) fn geometry_command_count(&self) -> u64 {
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

#[derive(Debug)]
pub(super) struct FallbackSource {
    pub(super) id: Option<String>,
    pub(super) pointer: String,
}

#[derive(Debug)]
pub(super) struct SemanticWorkingRecord {
    pub(super) source_order: u64,
    pub(super) event: &'static str,
    pub(super) element_id: Option<String>,
    pub(super) local_owner: Option<String>,
    pub(super) json_pointer: String,
    pub(super) bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ResolvedTransform {
    pub(super) translation_x: f64,
    pub(super) translation_y: f64,
    pub(super) angle_radians: f64,
    pub(super) center_x: f64,
    pub(super) center_y: f64,
}

pub(super) struct ElementLocalCrossing {
    pub(super) code: DiagnosticCode,
    pub(super) field: &'static str,
    pub(super) configured: ResourceValue,
    pub(super) value_kind: ValueKind,
    pub(super) attempted: ResourceValue,
    pub(super) pointer_field: Option<&'static str>,
    pub(super) message: &'static str,
}

impl ElementLocalCrossing {
    pub(super) fn diagnostic(
        self,
        format: OutputFormat,
        element_id: Option<&str>,
        element_pointer: &str,
    ) -> Result<CompatibilityDiagnostic, std::collections::TryReserveError> {
        self.diagnostic_with_recovery(format, element_id, element_pointer, Recovery::Omitted)
    }

    pub(super) fn diagnostic_with_recovery(
        self,
        format: OutputFormat,
        element_id: Option<&str>,
        element_pointer: &str,
        recovery: Recovery,
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
                recovery: Some(recovery),
            })),
            order: DiagnosticOrder::default(),
        })
    }
}

pub(super) fn outline_local_crossing(
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

pub(super) fn generation_local_crossing(
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

pub(super) fn logical_aligned_bytes(bytes: u64) -> u64 {
    bytes.saturating_add(63) / 64 * 64
}

pub(super) fn fallback_fits_local_limits(
    fallback: &crate::scene_surface::SurfaceFallback,
    limits: &crate::ResourceLimits,
) -> bool {
    let coordinate_limit = limits.max_coordinate_magnitude();
    if [fallback.x, fallback.y, fallback.width, fallback.height]
        .into_iter()
        .any(|value| value.abs() > coordinate_limit)
    {
        return false;
    }
    let geometry_segments = if fallback.width >= 2.0 && fallback.height >= 2.0 {
        9_u64
    } else {
        5
    };
    geometry_segments <= u64::from(limits.max_geometry_segments_per_element())
        && geometry_segments.saturating_mul(32) <= limits.max_element_work_units()
}

pub(super) fn artifact_retention_delta(
    output_bytes: u64,
    codec_emission_work: u64,
) -> ResourceDelta {
    ResourceDelta {
        working_bytes: 512_u64.saturating_add(logical_aligned_bytes(output_bytes)),
        output_bytes,
        total_work_units: codec_emission_work.saturating_add(output_bytes),
        ..ResourceDelta::default()
    }
}
