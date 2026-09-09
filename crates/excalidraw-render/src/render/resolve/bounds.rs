//! The scene's layout bounds: the pass over every outline, frame name and
//! painted placeholder that settles the layout origin every canvas placement
//! subtracts and the logical size of the artifact.

use crate::normalization::{NormalizedElement, NormalizedElementKind};
use crate::portable_math::sin_cos;
use crate::relationships::ResolvedRelationshipGraph;
use crate::scene_surface::SurfaceFallback;

use super::super::frame_target::{
    relationship_frame_target, EffectiveFrameRendering, FrameViewport,
};
use super::super::geometry::{include_transformed_bounds, source_layout_bounds, GeneratedGeometry};
use super::super::text::{FrameName, FRAME_NAME_BOX_HEIGHT, FRAME_NAME_OFFSET_Y};

/// The scene's layout bounds as `(min_x, min_y, max_x, max_y)`: the frame
/// viewport of a frame export, or the rotated extents of every outline frame
/// clipping leaves visible, every frame name, and every painted fallback that
/// is not a clipped frame child. `None` when an element's normalized style is
/// outside the focused case this stage implements; infinite bounds mean the
/// scene has nothing to measure.
pub(super) fn scene_bounds(
    outlines: &[(NormalizedElement, Option<GeneratedGeometry>)],
    fallbacks: &[SurfaceFallback],
    frame_names: &[(usize, FrameName)],
    relationships: &ResolvedRelationshipGraph,
    frame_rendering: EffectiveFrameRendering,
    viewport: Option<FrameViewport>,
) -> Option<(f64, f64, f64, f64)> {
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
    for (source, geometry) in outlines {
        if source.stroke_width < 0.0
            || source.roughness < 0.0
            || !(0.0..=100.0).contains(&source.opacity)
            || source.seed == 0
        {
            return None;
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
                source.y - FRAME_NAME_OFFSET_Y - FRAME_NAME_BOX_HEIGHT,
                name.width,
                FRAME_NAME_BOX_HEIGHT,
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
        let (local_min_x, local_min_y, local_max_x, local_max_y) = geometry.as_ref().map_or_else(
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
        let (sin, cos) = sin_cos(source.angle);
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
    for fallback in fallbacks {
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
                    .and_then(|node| relationship_frame_target(node.source_index, relationships))
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
    Some((min_x, min_y, max_x, max_y))
}
