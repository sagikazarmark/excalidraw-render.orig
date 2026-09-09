//! Outline resolution: the placement, stroke color and base work units every
//! generated geometry shares, then one `ResolvedOutline` per Rough drawing —
//! with the generated label of an embeddable placeholder and the bound-text
//! mask of a container — or one `ResolvedFreedraw` per freedraw path.

use crate::color::{self, CanonicalColor};
use crate::font_metadata::FamilyMetrics;
use crate::normalization::{NormalizedElement, NormalizedElementKind};
use crate::OutputFormat;

use super::super::geometry::{GeneratedFreedraw, GeneratedGeometry};
use super::super::rough::Drawing;
use super::super::text::{OfficialFont, ResolvedText, TextLayout};
use super::text::{layout_official_text, official_font_index};
use super::{
    label_error, FrameMembership, ResolveContext, ResolveSceneError, ResolvedElement,
    ResolvedFreedraw, ResolvedLabelMask, ResolvedOutline, ResolvedTransform, StrokeStyle,
};

/// Excalidraw creates the offline placeholder label as Helvetica text, so the
/// label is shaped with the Liberation Sans substitute and positioned by
/// Helvetica's metadata metrics.
const EMBEDDABLE_LABEL_FONT_FAMILY: i32 = 2;

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

/// What the Rough and freedraw outlines share once their geometry is
/// generated: the canvas placement rotated about the geometry's center, the
/// stroke color for the mode, and the base work units the ledger charges for
/// the element.
struct OutlineCommon {
    transform: ResolvedTransform,
    stroke_color: CanonicalColor,
    base_element_work_units: u64,
}

/// One outline with generated geometry: the shared placement, stroke color and
/// base work units, then the Rough drawing or the freedraw path.
pub(super) fn resolve_geometry(
    source: NormalizedElement,
    geometry: GeneratedGeometry,
    context: &ResolveContext<'_>,
    official_fonts: &mut Vec<OfficialFont>,
    membership: FrameMembership,
) -> Result<ResolvedElement, ResolveSceneError> {
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
        .for_mode(context.request.color_mode());
    let common = OutlineCommon {
        transform,
        stroke_color,
        base_element_work_units: source.normalization_work_units
            + geometry_work_units
            + 8
            + source.repair_work_units,
    };
    match geometry {
        GeneratedGeometry::Rough(drawing) => Ok(ResolvedElement::Outline(resolve_outline(
            source,
            drawing,
            common,
            context,
            official_fonts,
            membership,
        )?)),
        GeneratedGeometry::Freedraw(freedraw) => Ok(ResolvedElement::Freedraw(resolve_freedraw(
            source, freedraw, common, context, membership,
        ))),
    }
}

/// The generated label of an embeddable placeholder — the sanitized link or
/// `IFrame element` — wrapped and laid out in the label font, with its color
/// (the stroke color, or black over a transparent stroke) and the vertical
/// offset that centers its line box in the element.
fn resolve_embeddable_label(
    source: &NormalizedElement,
    label: &str,
    context: &ResolveContext<'_>,
    official_fonts: &mut Vec<OfficialFont>,
) -> Result<(ResolvedText, CanonicalColor, f64), ResolveSceneError> {
    let font_size = embeddable_font_size(source.width, label);
    let font_index = official_font_index(official_fonts, EMBEDDABLE_LABEL_FONT_FAMILY, label)?;
    let wrapped_label = official_fonts[font_index]
        .wrap_embeddable_label(label, font_size, source.width - 20.0)
        .map_err(label_error)?;
    let laid_out = layout_official_text(
        official_fonts,
        font_index,
        &wrapped_label,
        TextLayout {
            width: source.width,
            font_size,
            line_height: EMBEDDABLE_LABEL_LINE_HEIGHT,
            text_align: "center",
            font_family: EMBEDDABLE_LABEL_FONT_FAMILY,
        },
        context.cjk_subset_trigger,
    )?;
    let line_count = laid_out.lines.len() as f64;
    let offset_y = (source.height - line_count * font_size * EMBEDDABLE_LABEL_LINE_HEIGHT) / 2.0;
    let label_color = color::parse_css(if source.stroke_color == "transparent" {
        "#000000"
    } else {
        &source.stroke_color
    })
    .ok_or(ResolveSceneError::InvalidColor)?
    .for_mode(context.request.color_mode());
    Ok((laid_out, label_color, offset_y))
}

/// One Rough outline — a shape, a linear element or an embeddable placeholder
/// — with its generated label laid out in the label font and its bound-text
/// mask placed on the canvas. A PNG export of a zero-extent embeddable draws
/// no label, as the canvas draws none.
fn resolve_outline(
    source: NormalizedElement,
    drawing: Drawing,
    common: OutlineCommon,
    context: &ResolveContext<'_>,
    official_fonts: &mut Vec<OfficialFont>,
    membership: FrameMembership,
) -> Result<ResolvedOutline, ResolveSceneError> {
    let source_index = source.source.source_index();
    let (generated_label, generated_label_color, generated_label_offset_y) =
        if context.request.format() == OutputFormat::Png
            && (source.width == 0.0 || source.height == 0.0)
        {
            (None, None, 0.0)
        } else if let NormalizedElementKind::Embeddable { label, .. } = &source.kind {
            let (laid_out, label_color, offset_y) =
                resolve_embeddable_label(&source, label, context, official_fonts)?;
            (Some(laid_out), Some(label_color), offset_y)
        } else {
            (None, None, 0.0)
        };
    Ok(ResolvedOutline {
        source_order: source.source_order,
        source: source.source,
        base_element_work_units: common.base_element_work_units,
        drawing,
        frame_clips: membership.clips,
        transform: common.transform,
        stroke_color: common.stroke_color,
        fill_color: (source.background_color != "transparent")
            .then(|| color::parse_css(&source.background_color))
            .flatten()
            .map(|color| color.for_mode(context.request.color_mode())),
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
        opacity: source.opacity / 100.0 * membership.opacity,
        element_opacity: source.opacity / 100.0,
        background_color: context.background_color,
        label_mask: context
            .bound_text_masks
            .iter()
            .find(|mask| mask.container_source_index == source_index)
            .map(|mask| ResolvedLabelMask {
                x: mask.x - context.min_x + context.request.padding(),
                y: mask.y - context.min_y + context.request.padding(),
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
    })
}

/// One freedraw outline: the generated path and its optional background fill,
/// which nothing about the element can fail to place.
fn resolve_freedraw(
    source: NormalizedElement,
    freedraw: GeneratedFreedraw,
    common: OutlineCommon,
    context: &ResolveContext<'_>,
    membership: FrameMembership,
) -> ResolvedFreedraw {
    ResolvedFreedraw {
        source_order: source.source_order,
        source: source.source,
        base_element_work_units: common.base_element_work_units,
        path: freedraw.path,
        background_fill: freedraw.background_fill,
        frame_clips: membership.clips,
        transform: common.transform,
        color: common.stroke_color,
        background_color: (source.background_color != "transparent")
            .then(|| color::parse_css(&source.background_color))
            .flatten()
            .map(|color| color.for_mode(context.request.color_mode())),
        source_stroke_width: source.stroke_width,
        opacity: source.opacity / 100.0 * membership.opacity,
        link: source.link,
        width: source.width,
        height: source.height,
    }
}
