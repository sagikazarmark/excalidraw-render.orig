//! Nested SVG in PDF: the planning analysis that finds every filter occurrence
//! in a sanitized `usvg` tree and turns the ones the asset's effect census
//! attributes into `PdfRasterClosure`s — the smallest page regions the raster
//! fallback must paint — and the bounded svg2pdf adapter that lowers each
//! sanitized tree to a Form XObject chunk, renumbers its objects into the plan's
//! layout and canonicalizes the writer's dictionary spelling so the artifact
//! bytes stay stable across the pinned dependency.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::portable_math::sin_cos;
use crate::{FiniteValue, LogicalBounds, NestedPath, NestedPathSegment, NestedSegmentKind};

use super::super::asset::svg::EffectCause;
use super::super::resolve::{ResolvedImageAsset, ResolvedImageProjection, ResolvedTransform};
use super::super::{FormatEncodeError, SvgTreeCache};
use super::{pdf_size_error, visible_rect, PdfElement, PdfRasterClosure, PdfSvgResource};

#[derive(Debug)]
struct FilterOccurrence {
    filter_id: String,
    usage_id: String,
    marker: String,
    bounds: (f64, f64, f64, f64),
    parent: Option<usize>,
}

pub(super) struct PdfSvgChunk {
    pub(super) chunk: pdf_writer::Chunk,
    pub(super) root: pdf_writer::Ref,
    pub(super) bytes: Vec<u8>,
    object_offsets: Vec<(usize, usize)>,
}

pub(super) fn collect_raster_closures(
    elements: &[PdfElement],
    svg_trees: &SvgTreeCache,
    final_scale: f64,
    logical_width: f64,
    logical_height: f64,
) -> Result<Vec<PdfRasterClosure>, FormatEncodeError> {
    let mut closures: Vec<PdfRasterClosure> = Vec::new();
    for (element_index, element) in elements.iter().enumerate() {
        let PdfElement::Image {
            source,
            asset,
            transform,
            frame_clips,
            width,
            height,
            source_rect,
            projection,
            scale,
            ..
        } = element
        else {
            continue;
        };
        if asset.svg_effects.is_empty() {
            continue;
        }
        let tree = svg_trees
            .get(&asset.identity)
            .map(Arc::as_ref)
            .ok_or_else(|| {
                FormatEncodeError::Encoding("PDF effect source lost its sanitized tree".to_owned())
            })?;
        let mut occurrences = Vec::new();
        collect_filter_occurrences(tree.root(), None, &mut occurrences);
        let mut occurrence_closures: Vec<Option<usize>> = Vec::new();
        occurrence_closures.resize(occurrences.len(), None);
        let mut matched_occurrences = BTreeMap::<String, usize>::new();
        for (occurrence_index, occurrence) in occurrences.iter().enumerate() {
            let exact_usage = asset.svg_effects.iter().find(|effect| {
                !occurrence.usage_id.is_empty() && effect.usage_id == occurrence.usage_id
            });
            let candidates = asset
                .svg_effects
                .iter()
                .filter(|effect| {
                    effect.marker == occurrence.marker || effect.filter_id == occurrence.filter_id
                })
                .collect::<Vec<_>>();
            let key = if occurrence.marker.is_empty() {
                occurrence.filter_id.clone()
            } else {
                occurrence.marker.clone()
            };
            let matched = matched_occurrences.entry(key).or_default();
            let effect = exact_usage.or_else(|| {
                (!candidates.is_empty()).then(|| candidates[*matched % candidates.len()])
            });
            *matched = matched.saturating_add(1);
            let Some(effect) = effect else {
                continue;
            };
            if let Some(parent_closure) = occurrence
                .parent
                .and_then(|parent| occurrence_closures.get(parent).copied().flatten())
            {
                let closure = closures
                    .get_mut(parent_closure)
                    .ok_or_else(pdf_size_error)?;
                closure
                    .causes
                    .extend(effect.causes.iter().map(effect_cause_path));
                closure.unisolatable |= effect.unisolatable;
                occurrence_closures[occurrence_index] = Some(parent_closure);
                continue;
            }
            let (x, y, right, bottom) = occurrence.bounds;
            let viewport_right = f64::from(asset.width);
            let viewport_bottom = f64::from(asset.height);
            let x = x.max(0.0).max(source_rect.x);
            let y = y.max(0.0).max(source_rect.y);
            let right = right
                .min(viewport_right)
                .min(source_rect.x + source_rect.width);
            let bottom = bottom
                .min(viewport_bottom)
                .min(source_rect.y + source_rect.height);
            if right <= x || bottom <= y {
                continue;
            }
            let local = map_asset_bounds(
                (x, y, right, bottom),
                projection,
                *width,
                *height,
                asset,
                *scale,
            );
            let (x, y, right, bottom) = transform_bounds(local, *transform);
            let mut x = x.max(0.0);
            let mut y = y.max(0.0);
            let mut right = right.min(logical_width);
            let mut bottom = bottom.min(logical_height);
            for clip in frame_clips {
                let Some(clip_bounds) = clip.path.cubic_bounds() else {
                    right = x;
                    bottom = y;
                    break;
                };
                let clip_bounds = transform_bounds(clip_bounds, clip.transform);
                x = x.max(clip_bounds.0);
                y = y.max(clip_bounds.1);
                right = right.min(clip_bounds.2);
                bottom = bottom.min(clip_bounds.3);
            }
            if right <= x || bottom <= y {
                continue;
            }
            let pixel_x = (x * final_scale).floor();
            let pixel_y = (y * final_scale).floor();
            let pixel_right = (right * final_scale).ceil();
            let pixel_bottom = (bottom * final_scale).ceil();
            let pixel_width = (pixel_right - pixel_x).max(1.0) as u64;
            let pixel_height = (pixel_bottom - pixel_y).max(1.0) as u64;
            let logical_bounds = LogicalBounds {
                x: FiniteValue::new(pixel_x / final_scale).ok_or_else(pdf_size_error)?,
                y: FiniteValue::new(pixel_y / final_scale).ok_or_else(pdf_size_error)?,
                width: FiniteValue::new(pixel_width as f64 / final_scale)
                    .ok_or_else(pdf_size_error)?,
                height: FiniteValue::new(pixel_height as f64 / final_scale)
                    .ok_or_else(pdf_size_error)?,
            };
            let causes = effect
                .causes
                .iter()
                .map(effect_cause_path)
                .collect::<Vec<_>>();
            closures.push(PdfRasterClosure {
                element_id: source.id().to_owned(),
                json_pointer: asset.svg_fallback_pointer.clone().unwrap_or_default(),
                source_order: u64::try_from(element_index).unwrap_or(u64::MAX),
                source_position: effect.node_ordinal,
                logical_bounds,
                pixel_width,
                pixel_height,
                causes,
                unisolatable: effect.unisolatable,
            });
            occurrence_closures[occurrence_index] = Some(closures.len() - 1);
        }
    }
    Ok(closures)
}

fn collect_filter_occurrences(
    group: &usvg::Group,
    parent: Option<usize>,
    occurrences: &mut Vec<FilterOccurrence>,
) {
    let mut child_parent = parent;
    if !group.filters().is_empty()
        && group.has_children()
        && group.bounding_box().width() > 0.0
        && group.bounding_box().height() > 0.0
    {
        let rect = group.abs_layer_bounding_box();
        let mut bounds = (
            f64::from(rect.x()),
            f64::from(rect.y()),
            f64::from(rect.right()),
            f64::from(rect.bottom()),
        );
        if let Some(clip) = group.clip_path() {
            let Some(clip_bounds) = nested_clip_bounds(clip) else {
                return;
            };
            bounds.0 = bounds.0.max(clip_bounds.0);
            bounds.1 = bounds.1.max(clip_bounds.1);
            bounds.2 = bounds.2.min(clip_bounds.2);
            bounds.3 = bounds.3.min(clip_bounds.3);
            if bounds.2 <= bounds.0 || bounds.3 <= bounds.1 {
                return;
            }
        }
        for filter in group.filters() {
            occurrences.push(FilterOccurrence {
                filter_id: filter.id().to_owned(),
                usage_id: group.id().to_owned(),
                marker: filter
                    .primitives()
                    .first()
                    .map(|primitive| primitive.result().to_owned())
                    .unwrap_or_default(),
                bounds,
                parent,
            });
            child_parent = Some(occurrences.len() - 1);
        }
    }
    for child in group.children() {
        if let usvg::Node::Group(child) = child {
            collect_filter_occurrences(child, child_parent, occurrences);
        }
        child.subroots(|subroot| {
            collect_filter_occurrences(subroot, child_parent, occurrences);
        });
    }
}

fn nested_clip_bounds(clip: &usvg::ClipPath) -> Option<(f64, f64, f64, f64)> {
    if clip.root().bounding_box().width() == 0.0 || clip.root().bounding_box().height() == 0.0 {
        return None;
    }
    let rect = clip
        .root()
        .abs_layer_bounding_box()
        .transform(clip.transform())?;
    let mut bounds = (
        f64::from(rect.x()),
        f64::from(rect.y()),
        f64::from(rect.right()),
        f64::from(rect.bottom()),
    );
    if let Some(parent) = clip.clip_path() {
        let parent = nested_clip_bounds(parent)?;
        bounds.0 = bounds.0.max(parent.0);
        bounds.1 = bounds.1.max(parent.1);
        bounds.2 = bounds.2.min(parent.2);
        bounds.3 = bounds.3.min(parent.3);
        if bounds.2 <= bounds.0 || bounds.3 <= bounds.1 {
            return None;
        }
    }
    Some(bounds)
}

/// Maps asset-pixel bounds into the element's space the way the content
/// matrix draws the asset: through the projection's content rectangle,
/// mirrored about the element's unrounded center when flipped, and bounded by
/// the visible rectangle.
fn map_asset_bounds(
    bounds: (f64, f64, f64, f64),
    projection: &ResolvedImageProjection,
    width: f64,
    height: f64,
    asset: &ResolvedImageAsset,
    scale: [i8; 2],
) -> (f64, f64, f64, f64) {
    let (content_x, content_y, content_width, content_height) = projection.content_rect();
    let map_x = |value: f64| {
        let value = content_x + value / f64::from(asset.width) * content_width;
        if scale[0] < 0 {
            width - value
        } else {
            value
        }
    };
    let map_y = |value: f64| {
        let value = content_y + value / f64::from(asset.height) * content_height;
        if scale[1] < 0 {
            height - value
        } else {
            value
        }
    };
    let (x1, x2) = (map_x(bounds.0), map_x(bounds.2));
    let (y1, y2) = (map_y(bounds.1), map_y(bounds.3));
    let (visible_x, visible_y, visible_width, visible_height) =
        visible_rect(projection, width, height, scale);
    (
        x1.min(x2).max(visible_x),
        y1.min(y2).max(visible_y),
        x1.max(x2).min(visible_x + visible_width),
        y1.max(y2).min(visible_y + visible_height),
    )
}

fn transform_bounds(
    bounds: (f64, f64, f64, f64),
    transform: ResolvedTransform,
) -> (f64, f64, f64, f64) {
    let (sin, cos) = sin_cos(transform.angle_radians);
    let offset_x = transform.translation_x + transform.center_x - cos * transform.center_x
        + sin * transform.center_y;
    let offset_y = transform.translation_y + transform.center_y
        - sin * transform.center_x
        - cos * transform.center_y;
    let point = |x: f64, y: f64| (cos * x - sin * y + offset_x, sin * x + cos * y + offset_y);
    let points = [
        point(bounds.0, bounds.1),
        point(bounds.2, bounds.1),
        point(bounds.2, bounds.3),
        point(bounds.0, bounds.3),
    ];
    points.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), (x, y)| {
            (min_x.min(*x), min_y.min(*y), max_x.max(*x), max_y.max(*y))
        },
    )
}

fn effect_cause_path(cause: &EffectCause) -> NestedPath {
    let mut segments = Vec::new();
    for (node, attribute) in &cause.slots {
        segments.push(NestedPathSegment {
            kind: NestedSegmentKind::Node,
            ordinal: *node,
            name: None,
        });
        segments.push(NestedPathSegment {
            kind: NestedSegmentKind::Attribute,
            ordinal: *attribute,
            name: Some("href".to_owned()),
        });
    }
    segments.push(NestedPathSegment {
        kind: NestedSegmentKind::Node,
        ordinal: cause.node_ordinal,
        name: None,
    });
    if let Some((ordinal, name)) = &cause.attribute {
        segments.push(NestedPathSegment {
            kind: if cause.effect_input {
                NestedSegmentKind::EffectInput
            } else {
                NestedSegmentKind::Attribute
            },
            ordinal: *ordinal,
            name: Some(name.clone()),
        });
    }
    NestedPath { segments }
}

pub(super) fn svg_chunks(
    resources: &[PdfSvgResource],
    svg_trees: &SvgTreeCache,
) -> Result<Vec<PdfSvgChunk>, FormatEncodeError> {
    let mut chunks = Vec::new();
    chunks.try_reserve_exact(resources.len()).map_err(|_| {
        FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_resource",
            subject: "pdf_svg_chunks",
        }
    })?;
    for resource in resources {
        let tree = svg_trees
            .get(&resource.asset.identity)
            .map(Arc::as_ref)
            .ok_or_else(|| {
                FormatEncodeError::Encoding("PDF SVG resource lost its sanitized tree".to_owned())
            })?;
        let (chunk, root) = svg2pdf::to_chunk(
            tree,
            svg2pdf::ConversionOptions {
                compress: false,
                raster_scale: resource.raster_scale as f32,
                embed_text: false,
                ..svg2pdf::ConversionOptions::default()
            },
        )
        .map_err(|error| {
            FormatEncodeError::Encoding(format!("failed to lower sanitized SVG to PDF: {error}"))
        })?;
        chunks.push(PdfSvgChunk {
            chunk,
            root,
            bytes: Vec::new(),
            object_offsets: Vec::new(),
        });
    }
    Ok(chunks)
}

pub(super) fn renumber_svg_chunks(
    chunks: Vec<PdfSvgChunk>,
    first_object: usize,
) -> Result<Vec<PdfSvgChunk>, FormatEncodeError> {
    let total_objects = chunks
        .iter()
        .try_fold(0_usize, |count, chunk| {
            count.checked_add(chunk.chunk.refs().len())
        })
        .ok_or_else(pdf_size_error)?;
    let last_object = first_object
        .checked_add(total_objects)
        .ok_or_else(pdf_size_error)?;
    if last_object > i32::MAX as usize {
        return Err(pdf_size_error());
    }
    let mut next = i32::try_from(first_object).map_err(|_| pdf_size_error())?;
    let mut renumbered = Vec::new();
    renumbered.try_reserve_exact(chunks.len()).map_err(|_| {
        FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_resource",
            subject: "pdf_svg_chunks",
        }
    })?;
    for resource in chunks {
        let mut mapping = BTreeMap::new();
        let chunk = resource.chunk.renumber(|old| {
            *mapping.entry(old).or_insert_with(|| {
                let allocated = pdf_writer::Ref::new(next);
                next += 1;
                allocated
            })
        });
        let root = *mapping.get(&resource.root).ok_or_else(pdf_size_error)?;
        let (bytes, object_offsets) = canonicalize_pdf_chunk(&chunk)?;
        renumbered.push(PdfSvgChunk {
            chunk,
            root,
            bytes,
            object_offsets,
        });
    }
    Ok(renumbered)
}

pub(super) fn append_chunk(
    pdf: &mut Vec<u8>,
    offsets: &mut [usize],
    resource: &PdfSvgChunk,
) -> Result<(), FormatEncodeError> {
    for &(object, position) in &resource.object_offsets {
        let index = object.checked_sub(1).ok_or_else(pdf_size_error)?;
        *offsets.get_mut(index).ok_or_else(pdf_size_error)? =
            pdf.len().checked_add(position).ok_or_else(pdf_size_error)?;
    }
    pdf.extend_from_slice(&resource.bytes);
    Ok(())
}

#[allow(clippy::type_complexity)]
fn canonicalize_pdf_chunk(
    chunk: &pdf_writer::Chunk,
) -> Result<(Vec<u8>, Vec<(usize, usize)>), FormatEncodeError> {
    let bytes = chunk.as_bytes();
    let mut canonical = Vec::with_capacity(bytes.len());
    let mut offsets = Vec::new();
    offsets.try_reserve_exact(chunk.refs().len()).map_err(|_| {
        FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_svg_object_offsets",
        }
    })?;
    let mut cursor = 0_usize;
    for reference in chunk.refs() {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            canonical.push(bytes[cursor]);
            cursor += 1;
        }
        let marker = format!("{} 0 obj\n", reference.get());
        if !bytes[cursor..].starts_with(marker.as_bytes()) {
            return Err(FormatEncodeError::Encoding(
                "PDF SVG chunk object order did not match its reference table".to_owned(),
            ));
        }
        offsets.push((
            usize::try_from(reference.get()).map_err(|_| pdf_size_error())?,
            canonical.len(),
        ));
        canonical.extend_from_slice(marker.as_bytes());
        cursor += marker.len();

        let relative_end = find_bytes(&bytes[cursor..], b"endobj\n").ok_or_else(|| {
            FormatEncodeError::Encoding("PDF SVG chunk object was unterminated".to_owned())
        })?;
        let relative_stream = find_bytes(&bytes[cursor..cursor + relative_end], b"stream\n");
        if let Some(relative_stream) = relative_stream {
            let stream = cursor + relative_stream;
            canonicalize_pdf_text(&bytes[cursor..stream], &mut canonical);
            canonical.extend_from_slice(b"stream\n");
            let body = stream + b"stream\n".len();
            let length = pdf_stream_length(&bytes[cursor..stream])?;
            let body_end = body.checked_add(length).ok_or_else(pdf_size_error)?;
            let trailer = b"\nendstream\nendobj\n";
            let object_end = body_end
                .checked_add(trailer.len())
                .ok_or_else(pdf_size_error)?;
            if bytes.get(body_end..object_end) != Some(trailer.as_slice()) {
                return Err(FormatEncodeError::Encoding(
                    "PDF SVG stream length did not match its object boundary".to_owned(),
                ));
            }
            canonical.extend_from_slice(bytes.get(body..body_end).ok_or_else(pdf_size_error)?);
            canonical.extend_from_slice(trailer);
            cursor = object_end;
        } else {
            let object_end = cursor
                .checked_add(relative_end)
                .and_then(|end| end.checked_add(b"endobj\n".len()))
                .ok_or_else(pdf_size_error)?;
            canonicalize_pdf_text(&bytes[cursor..object_end], &mut canonical);
            cursor = object_end;
        }
    }
    canonicalize_pdf_text(&bytes[cursor..], &mut canonical);
    Ok((canonical, offsets))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn pdf_stream_length(dictionary: &[u8]) -> Result<usize, FormatEncodeError> {
    let marker = b"/Length ";
    let start = find_bytes(dictionary, marker)
        .and_then(|index| index.checked_add(marker.len()))
        .ok_or_else(|| {
            FormatEncodeError::Encoding("PDF SVG stream had no direct length".to_owned())
        })?;
    let digits = dictionary[start..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .copied()
        .collect::<Vec<_>>();
    if digits.is_empty() {
        return Err(FormatEncodeError::Encoding(
            "PDF SVG stream length was not an integer".to_owned(),
        ));
    }
    std::str::from_utf8(&digits)
        .ok()
        .and_then(|digits| digits.parse().ok())
        .ok_or_else(pdf_size_error)
}

fn canonicalize_pdf_text(bytes: &[u8], output: &mut Vec<u8>) {
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n').peekable();
    while let Some(line) = lines.next() {
        let indentation = line.iter().take_while(|byte| **byte == b' ').count();
        if line.get(indentation) != Some(&b'/') || line.ends_with(b"<<\n") {
            output.extend_from_slice(line);
            continue;
        }
        let mut run = vec![line];
        while let Some(next) = lines.peek().copied() {
            let next_indentation = next.iter().take_while(|byte| **byte == b' ').count();
            if next_indentation != indentation
                || next.get(next_indentation) != Some(&b'/')
                || next.ends_with(b"<<\n")
            {
                break;
            }
            if let Some(next) = lines.next() {
                run.push(next);
            }
        }
        run.sort_unstable();
        for line in run {
            output.extend_from_slice(line);
        }
    }
}
