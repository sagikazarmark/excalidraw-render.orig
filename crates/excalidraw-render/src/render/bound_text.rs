use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct BoundTextPlacement {
    pub(super) source_index: usize,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) angle: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BoundTextMaskSource {
    pub(super) container_source_index: usize,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

pub(super) fn resolve_bound_text_placements(
    outlines: &[(NormalizedElement, Option<GeneratedGeometry>)],
    relationships: &crate::relationships::ResolvedRelationshipGraph,
) -> Result<Vec<BoundTextPlacement>, ResolveSceneError> {
    let mut placements = Vec::new();
    placements.try_reserve(outlines.len())?;
    for (text, _) in outlines {
        if !matches!(text.kind, NormalizedElementKind::Text(_)) {
            continue;
        }
        let Some(container_index) = relationship_element_target(
            text.source.source_index(),
            crate::relationships::RelationshipKind::Mask,
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
                // A shape container's label paints at its own stored
                // coordinates. The official export does not re-derive them from
                // the container box and the align values: a label stored far
                // outside its container renders outside it, and expands the
                // scene bounds to include it. Arrow labels are different and
                // are still resolved to the arrow midpoint below.
                (text.x, text.y, container.angle)
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

pub(super) fn has_reciprocal_bound_text(
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

pub(super) fn resolve_bound_text_masks(
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
