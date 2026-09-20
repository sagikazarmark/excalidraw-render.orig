//! The PDF page content stream and the fallback form streams: one `q … Q`
//! group per `PdfElement` in paint order, with frame clips installed before the
//! element's `cm`, Rough paths spelled through `push_rough_number`, freedraw
//! quadratics lowered to cubics, text shown glyph by glyph through the
//! deterministic character codes `fonts` assigns, images invoked through their
//! shared XObjects, and compatibility placeholders drawn inline or through a
//! transparency-group form when they carry opacity.

use std::fmt::Write as _;

use crate::color::CanonicalColor;
use crate::portable_math::sin_cos;

use super::super::geometry::{Path, PathOperation, Point, FRAME_OUTLINE_STROKE_WIDTH};
use super::super::number_format::{format_pdf_number, push_rough_number};
use super::super::plan::PlannedElement;
use super::super::resolve::{
    decoration_fill_color, decoration_stroke_style, FallbackColors, ResolvedClip,
    ResolvedTransform, StrokeStyle,
};
use super::super::rough::{Drawing, FillKind};
use super::super::text::{ResolvedGlyph, ResolvedLabel};
use super::super::{freedraw, FormatEncodeError};
use super::fonts::{glyph_mapping_code, PdfGlyphMapping};
use super::{
    pdf_size_error, visible_rect, PdfElementKind, PdfImageResource, PdfPlan, PdfSvgResource,
    POINTS_PER_LOGICAL_PIXEL,
};

impl PdfPlan {
    pub(super) fn encode_fallback_forms(
        &self,
        fallback_mappings: &[PdfGlyphMapping],
    ) -> Result<Vec<String>, FormatEncodeError> {
        let form_count = self
            .elements
            .iter()
            .filter(|element| {
                matches!(element.kind, PdfElementKind::Fallback { .. }) && element.opacity != 1.0
            })
            .count();
        let mut forms = Vec::new();
        forms
            .try_reserve_exact(form_count)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_fallback_forms",
            })?;
        for element in &self.elements {
            let PlannedElement {
                width,
                height,
                opacity,
                kind: PdfElementKind::Fallback { label, .. },
                ..
            } = element
            else {
                continue;
            };
            if *opacity == 1.0 {
                continue;
            }
            let mut form = String::new();
            let capacity = label.as_ref().map_or(1_024, |label| {
                label
                    .path
                    .operations
                    .len()
                    .saturating_mul(160)
                    .saturating_add(label.text.len().saturating_mul(4))
                    .saturating_add(1_024)
            });
            form.try_reserve_exact(capacity)
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_workspace",
                    subject: "pdf_fallback_form_stream",
                })?;
            push_fallback_body(
                &mut form,
                *width,
                *height,
                label.as_ref(),
                self.fallback_colors,
                fallback_mappings,
            )?;
            forms.push(form);
        }
        Ok(forms)
    }

    pub(super) fn encode_content(
        &self,
        opacity_resources: &[f64],
        image_resources: &[PdfImageResource],
        svg_resources: &[PdfSvgResource],
        fallback_mappings: &[PdfGlyphMapping],
        official_mappings: &[Vec<PdfGlyphMapping>],
    ) -> Result<String, FormatEncodeError> {
        let mut capacity = 1_024_usize;
        for element in &self.elements {
            let element_bytes = match &element.kind {
                PdfElementKind::RoughOutline {
                    drawing,
                    generated_label,
                    ..
                } => drawing
                    .operation_count()
                    .checked_mul(200)
                    .and_then(|path_bytes| {
                        path_bytes.checked_add(generated_label.as_ref().map_or(512, |label| {
                            label.lines.iter().fold(1_024_usize, |size, line| {
                                size.saturating_add(line.glyphs.len().saturating_mul(96))
                            })
                        }))
                    })
                    .ok_or_else(pdf_size_error)?,
                PdfElementKind::Freedraw {
                    path,
                    background_fill,
                    ..
                } => path
                    .operation_count()
                    .saturating_add(
                        background_fill
                            .as_ref()
                            .map_or(0, |fill| fill.path.operations.len()),
                    )
                    .checked_mul(160)
                    .and_then(|path_bytes| path_bytes.checked_add(256))
                    .ok_or_else(pdf_size_error)?,
                PdfElementKind::Frame { name, .. } => name
                    .as_ref()
                    .map_or(512, |name| name.label.text.len().saturating_add(1_024)),
                PdfElementKind::Text { text, .. } => {
                    text.lines.iter().fold(512_usize, |size, line| {
                        size.saturating_add(line.text.len().saturating_mul(4))
                            .saturating_add(line.glyphs.len().saturating_mul(96))
                    })
                }
                PdfElementKind::Image { corner_clip, .. } => {
                    corner_clip.as_ref().map_or(512, |path| {
                        path.operations
                            .len()
                            .saturating_mul(160)
                            .saturating_add(512)
                    })
                }
                PdfElementKind::Fallback { label, .. } => label.as_ref().map_or(1_024, |label| {
                    label
                        .path
                        .operations
                        .len()
                        .saturating_mul(160)
                        .saturating_add(label.text.len().saturating_mul(4))
                        .saturating_add(1_024)
                }),
            };
            capacity = capacity
                .checked_add(element_bytes)
                .ok_or_else(pdf_size_error)?;
        }
        let mut content = String::new();
        content
            .try_reserve_exact(capacity)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_content_stream",
            })?;
        content.push_str("q\n");
        push_matrix(
            &mut content,
            self.scale * POINTS_PER_LOGICAL_PIXEL,
            0.0,
            0.0,
            -self.scale * POINTS_PER_LOGICAL_PIXEL,
            0.0,
            self.page_height,
        )?;
        if let Some(background) = self.background {
            if background.alpha() != 1.0 {
                content.push_str("q\n");
                push_opacity_resource(&mut content, opacity_resources, background.alpha())?;
            }
            content.push_str("/CS1 cs\n");
            push_color(&mut content, background.rgb_f64(), "scn")?;
            push_pdf_number(&mut content, 0.0, "PDF background x")?;
            content.push(' ');
            push_pdf_number(&mut content, 0.0, "PDF background y")?;
            content.push(' ');
            push_pdf_number(&mut content, self.logical_width, "PDF background width")?;
            content.push(' ');
            push_pdf_number(&mut content, self.logical_height, "PDF background height")?;
            content.push_str(" re\nf\n");
            if background.alpha() != 1.0 {
                content.push_str("Q\n");
            }
        }
        let mut fallback_form_index = 0_usize;
        for element in &self.elements {
            let PlannedElement {
                frame_clips,
                transform,
                opacity,
                link: _,
                width,
                height,
                kind,
            } = element;
            match kind {
                PdfElementKind::RoughOutline {
                    source,
                    drawing,
                    stroke_color,
                    fill_color,
                    stroke_width,
                    stroke_style,
                    source_stroke_width,
                    background_color,
                    label_mask,
                    generated_label,
                    generated_label_color,
                    generated_label_offset_y,
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF plan outline lost its source identity".to_owned(),
                        ));
                    }
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    if let Some(mask) = label_mask {
                        content.push_str("0 0 ");
                        push_pdf_number(
                            &mut content,
                            self.logical_width,
                            "PDF arrow mask outer width",
                        )?;
                        content.push(' ');
                        push_pdf_number(
                            &mut content,
                            self.logical_height,
                            "PDF arrow mask outer height",
                        )?;
                        content.push_str(" re\n");
                        push_pdf_number(&mut content, mask.x, "PDF arrow label mask x")?;
                        content.push(' ');
                        push_pdf_number(&mut content, mask.y, "PDF arrow label mask y")?;
                        content.push(' ');
                        push_pdf_number(&mut content, mask.width, "PDF arrow label mask width")?;
                        content.push(' ');
                        push_pdf_number(&mut content, mask.height, "PDF arrow label mask height")?;
                        content.push_str(" re\nW*\nn\n");
                    }
                    push_transform_matrix(&mut content, transform)?;
                    if let (Some(fill), Some(fill_color)) = (&drawing.fill, fill_color) {
                        content.push_str("q\n");
                        let fill_opacity = *opacity * fill_color.alpha();
                        if fill_opacity != 1.0 {
                            push_opacity_resource(&mut content, opacity_resources, fill_opacity)?;
                        }
                        match fill.kind {
                            FillKind::Solid => {
                                content.push_str("/CS1 cs\n");
                                push_color(&mut content, fill_color.rgb_f64(), "scn")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str(if drawing.shape.canvas_uses_even_odd_fill() {
                                    "f*\n"
                                } else {
                                    "f\n"
                                });
                            }
                            FillKind::Sketch => {
                                push_pdf_number(
                                    &mut content,
                                    *source_stroke_width / 2.0,
                                    "PDF fill weight",
                                )?;
                                content.push_str(" w\n1 J\n/CS1 CS\n");
                                push_color(&mut content, fill_color.rgb_f64(), "SCN")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str("S\n");
                            }
                        }
                        content.push_str("Q\n");
                    }
                    let stroke_opacity = *opacity * stroke_color.alpha();
                    if stroke_opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, stroke_opacity)?;
                    }
                    push_pdf_number(&mut content, *stroke_width, "PDF stroke width")?;
                    content.push_str(" w\n1 J\n");
                    match stroke_style {
                        StrokeStyle::Solid => {}
                        StrokeStyle::Dashed => {
                            content.push_str("[8 ");
                            push_pdf_number(
                                &mut content,
                                8.0 + source_stroke_width,
                                "PDF dash gap",
                            )?;
                            content.push_str("] 0 d\n");
                        }
                        StrokeStyle::Dotted => {
                            content.push_str("[1.5 ");
                            push_pdf_number(
                                &mut content,
                                6.0 + source_stroke_width,
                                "PDF dot gap",
                            )?;
                            content.push_str("] 0 d\n");
                        }
                    }
                    content.push_str("/CS1 CS\n");
                    push_color(&mut content, stroke_color.rgb_f64(), "SCN")?;
                    push_rough_path(&mut content, &drawing.outline)?;
                    content.push_str("S\n");
                    for decoration in &drawing.decorations {
                        let fill =
                            decoration_fill_color(decoration, *stroke_color, *background_color);
                        let (decoration_style, decoration_stroke_width) =
                            decoration_stroke_style(decoration, *source_stroke_width);
                        push_pdf_drawing(
                            &mut content,
                            &decoration.drawing,
                            *stroke_color,
                            Some(fill),
                            *stroke_width,
                            decoration_style,
                            decoration_stroke_width,
                            *opacity,
                            opacity_resources,
                        )?;
                    }
                    if let (Some(label), Some(color)) = (generated_label, generated_label_color) {
                        content.push_str("q\n1 0 0 1 0 ");
                        push_pdf_number(
                            &mut content,
                            *generated_label_offset_y,
                            "PDF embeddable label offset",
                        )?;
                        content.push_str(" cm\n");
                        let effective_opacity = color.alpha();
                        if effective_opacity != 1.0 {
                            push_opacity_resource(
                                &mut content,
                                opacity_resources,
                                effective_opacity,
                            )?;
                        }
                        content.push_str("/CS1 cs\n");
                        push_color(&mut content, color.rgb_f64(), "scn")?;
                        for line in &label.lines {
                            content.push_str("/Span << /ActualText <FEFF");
                            push_utf16be_hex(&mut content, &line.text)?;
                            content.push_str("> >> BDC\nBT\n");
                            if line.runs.is_empty() {
                                push_pdf_text_run(
                                    &mut content,
                                    label.font_index,
                                    label.font_size,
                                    &line.glyphs,
                                    official_mappings,
                                )?;
                            } else {
                                for run in &line.runs {
                                    let glyphs = line
                                        .glyphs
                                        .get(run.glyph_start..run.glyph_end)
                                        .ok_or_else(|| {
                                            FormatEncodeError::Encoding(
                                                "PDF embeddable run has invalid glyph bounds"
                                                    .to_owned(),
                                            )
                                        })?;
                                    push_pdf_text_run(
                                        &mut content,
                                        run.font_index,
                                        label.font_size,
                                        glyphs,
                                        official_mappings,
                                    )?;
                                }
                            }
                            content.push_str("ET\nEMC\n");
                        }
                        content.push_str("Q\n");
                    }
                    content.push_str("Q\n");
                }
                PdfElementKind::Freedraw {
                    source,
                    path,
                    background_fill,
                    color,
                    background_color,
                    source_stroke_width,
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF freedraw lost its source identity".to_owned(),
                        ));
                    }
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_transform_matrix(&mut content, transform)?;
                    if let (Some(fill), Some(background_color)) =
                        (background_fill, background_color)
                    {
                        content.push_str("q\n");
                        let fill_opacity = *opacity * background_color.alpha();
                        if fill_opacity != 1.0 {
                            push_opacity_resource(&mut content, opacity_resources, fill_opacity)?;
                        }
                        match fill.kind {
                            FillKind::Solid => {
                                content.push_str("/CS1 cs\n");
                                push_color(&mut content, background_color.rgb_f64(), "scn")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str("f*\n");
                            }
                            FillKind::Sketch => {
                                push_pdf_number(
                                    &mut content,
                                    *source_stroke_width / 2.0,
                                    "PDF freedraw fill weight",
                                )?;
                                content.push_str(" w\n1 J\n/CS1 CS\n");
                                push_color(&mut content, background_color.rgb_f64(), "SCN")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str("S\n");
                            }
                        }
                        content.push_str("Q\n");
                    }
                    if path.is_empty() {
                        content.push_str("Q\n");
                        continue;
                    }
                    let effective_opacity = *opacity * color.alpha();
                    if effective_opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, effective_opacity)?;
                    }
                    content.push_str("/CS1 cs\n");
                    push_color(&mut content, color.rgb_f64(), "scn")?;
                    push_freedraw_path(&mut content, path)?;
                    content.push_str("f\nQ\n");
                }
                PdfElementKind::Frame {
                    source,
                    name,
                    outline,
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF frame lost its source identity".to_owned(),
                        ));
                    }
                    // The name is its own text element: unrotated, unclipped
                    // and fully opaque, painted before the frame.
                    if let Some(name) = name {
                        content.push_str("q\n");
                        push_transform_matrix(&mut content, &name.transform)?;
                        content.push_str("/Span << /ActualText <FEFF");
                        push_utf16be_hex(&mut content, &name.label.text)?;
                        content.push_str("> >> BDC\n");
                        push_pdf_label_text(
                            &mut content,
                            &name.label,
                            name.color,
                            fallback_mappings,
                        )?;
                        content.push_str("EMC\nQ\n");
                    }
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_transform_matrix(&mut content, transform)?;
                    if *opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, *opacity)?;
                    }
                    if let Some(outline) = outline {
                        push_pdf_number(
                            &mut content,
                            FRAME_OUTLINE_STROKE_WIDTH,
                            "PDF frame outline stroke width",
                        )?;
                        content.push_str(" w\n1 J\n1 j\n/CS1 CS\n");
                        // PDF follows the reference SVG's outline color, which
                        // does not recolor magic frames.
                        push_color(&mut content, outline.stroke_color.rgb_f64(), "SCN")?;
                        push_rough_path(&mut content, &outline.path)?;
                        content.push_str("S\n");
                    }
                    content.push_str("Q\n");
                }
                PdfElementKind::Text {
                    source,
                    text,
                    color,
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF text lost its source identity".to_owned(),
                        ));
                    }
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_transform_matrix(&mut content, transform)?;
                    let effective_opacity = *opacity * color.alpha();
                    if effective_opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, effective_opacity)?;
                    }
                    content.push_str("/CS1 cs\n");
                    push_color(&mut content, color.rgb_f64(), "scn")?;
                    for line in &text.lines {
                        content.push_str("/Span << /ActualText <FEFF");
                        push_utf16be_hex(&mut content, &line.text)?;
                        content.push_str("> >> BDC\nBT\n");
                        if line.runs.is_empty() {
                            push_pdf_text_run(
                                &mut content,
                                text.font_index,
                                text.font_size,
                                &line.glyphs,
                                official_mappings,
                            )?;
                        } else {
                            for run in &line.runs {
                                let glyphs = line
                                    .glyphs
                                    .get(run.glyph_start..run.glyph_end)
                                    .ok_or_else(|| {
                                        FormatEncodeError::Encoding(
                                            "PDF text run has invalid glyph bounds".to_owned(),
                                        )
                                    })?;
                                push_pdf_text_run(
                                    &mut content,
                                    run.font_index,
                                    text.font_size,
                                    glyphs,
                                    official_mappings,
                                )?;
                            }
                        }
                        content.push_str("ET\nEMC\n");
                    }
                    content.push_str("Q\n");
                }
                PdfElementKind::Image {
                    source_order: _,
                    source,
                    asset,
                    dark_mode_filter: _,
                    corner_clip,
                    source_rect: _,
                    projection,
                    scale,
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF image lost its source identity".to_owned(),
                        ));
                    }
                    let (resource_prefix, resource) = if asset.sanitized_svg.is_some() {
                        (
                            "Svg",
                            svg_resources.iter().position(|candidate| {
                                if asset.svg_effects.is_empty() {
                                    candidate.asset.identity == asset.identity
                                } else {
                                    candidate.source_index == source.source_index()
                                }
                            }),
                        )
                    } else {
                        (
                            "Im",
                            image_resources
                                .iter()
                                .position(|candidate| candidate.asset.identity == asset.identity),
                        )
                    };
                    let resource = resource.ok_or_else(|| {
                        FormatEncodeError::Encoding(
                            "PDF image resource has no semantic owner".to_owned(),
                        )
                    })?;
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_transform_matrix(&mut content, transform)?;
                    if *opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, *opacity)?;
                    }
                    // The element-space clips precede the content matrix, so
                    // a rounded corner stays at the element's corner however
                    // the source is mapped into it. Image clips keep full
                    // precision (spec 8.6): the rough two-decimal rounding does
                    // not apply to them, and only the viewport carries the
                    // reference's integer rounding.
                    if let Some(path) = corner_clip {
                        push_mapped_clip(&mut content, path, |x, y| (x, y), "PDF image clip")?;
                    }
                    let flip_x = scale[0] < 0;
                    let flip_y = scale[1] < 0;
                    let (content_x, content_y, content_width, content_height) =
                        projection.content_rect();
                    // A cropped image overflows the region the reference leaves
                    // visible; an uncropped one fills its viewport, so the
                    // clip only restates the image's own bounds and yields to
                    // a corner clip.
                    if projection.crop.is_some() || corner_clip.is_none() {
                        let (x, y, visible_width, visible_height) =
                            visible_rect(projection, *width, *height, *scale);
                        push_rect_clip(
                            &mut content,
                            x,
                            y,
                            visible_width,
                            visible_height,
                            "PDF image viewport",
                        )?;
                    }
                    // The asset fills the content rectangle, mirrored about the
                    // element's unrounded center when flipped.
                    let (image_scale_x, image_offset_x) = if flip_x {
                        (-content_width, *width - content_x)
                    } else {
                        (content_width, content_x)
                    };
                    let (mut image_scale_y, mut image_offset_y) = if flip_y {
                        (-content_height, *height - content_y)
                    } else {
                        (content_height, content_y)
                    };
                    // PDF image samples start at the unit image's upper edge;
                    // svg2pdf Forms likewise invert SVG's Y axis. Compensate
                    // both within the content rectangle after element flips.
                    image_offset_y += image_scale_y;
                    image_scale_y = -image_scale_y;
                    push_matrix(
                        &mut content,
                        image_scale_x,
                        0.0,
                        0.0,
                        image_scale_y,
                        image_offset_x,
                        image_offset_y,
                    )?;
                    writeln!(content, "/{resource_prefix}{resource} Do").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write PDF image invocation".to_owned(),
                        )
                    })?;
                    content.push_str("Q\n");
                }
                PdfElementKind::Fallback { source, label } => {
                    if source.pointer.is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF fallback lost its source location".to_owned(),
                        ));
                    }
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_transform_matrix(&mut content, transform)?;
                    if *opacity != 1.0 {
                        let resource = opacity_resources
                            .iter()
                            .position(|candidate| candidate.to_bits() == opacity.to_bits())
                            .ok_or_else(|| {
                                FormatEncodeError::Encoding(
                                    "PDF fallback opacity resource has no owner".to_owned(),
                                )
                            })?;
                        writeln!(content, "/GS{} gs", resource + 1).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write PDF fallback opacity resource".to_owned(),
                            )
                        })?;
                        fallback_form_index = fallback_form_index.saturating_add(1);
                        writeln!(content, "/FB{fallback_form_index} Do").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write PDF fallback form invocation".to_owned(),
                            )
                        })?;
                        content.push_str("Q\n");
                        continue;
                    }
                    push_fallback_body(
                        &mut content,
                        *width,
                        *height,
                        label.as_ref(),
                        self.fallback_colors,
                        fallback_mappings,
                    )?;
                    content.push_str("Q\n");
                }
            }
        }
        content.push('Q');
        Ok(content)
    }
}

fn push_pdf_text_run(
    content: &mut String,
    font_index: usize,
    font_size: f64,
    glyphs: &[ResolvedGlyph],
    official_mappings: &[Vec<PdfGlyphMapping>],
) -> Result<(), FormatEncodeError> {
    content.push_str("/F");
    write!(content, "{} ", font_index + 2).map_err(|_| {
        FormatEncodeError::Encoding("failed to write PDF text font resource".to_owned())
    })?;
    push_pdf_number(content, font_size, "PDF text font size")?;
    content.push_str(" Tf\n");
    let mappings = official_mappings.get(font_index).ok_or_else(|| {
        FormatEncodeError::Encoding("PDF text references an unknown font resource".to_owned())
    })?;
    for glyph in glyphs {
        content.push_str("1 0 0 -1 ");
        push_pdf_number(content, glyph.x, "PDF text glyph x")?;
        content.push(' ');
        push_pdf_number(content, glyph.y, "PDF text glyph y")?;
        content.push_str(" Tm\n<");
        let code = glyph_mapping_code(mappings, glyph)?;
        write!(content, "{code:04X}").map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF text glyph ID".to_owned())
        })?;
        content.push_str("> Tj\n");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_pdf_drawing(
    output: &mut String,
    drawing: &Drawing,
    stroke_color: CanonicalColor,
    fill_color: Option<CanonicalColor>,
    stroke_width: f64,
    stroke_style: StrokeStyle,
    source_stroke_width: f64,
    opacity: f64,
    opacity_resources: &[f64],
) -> Result<(), FormatEncodeError> {
    output.push_str("q\n");
    if let (Some(fill), Some(fill_color)) = (&drawing.fill, fill_color) {
        output.push_str("q\n");
        let fill_opacity = opacity * fill_color.alpha();
        if fill_opacity != 1.0 {
            push_opacity_resource(output, opacity_resources, fill_opacity)?;
        }
        match fill.kind {
            FillKind::Solid => {
                output.push_str("/CS1 cs\n");
                push_color(output, fill_color.rgb_f64(), "scn")?;
                push_rough_path(output, &fill.path)?;
                output.push_str(if drawing.shape.canvas_uses_even_odd_fill() {
                    "f*\n"
                } else {
                    "f\n"
                });
            }
            FillKind::Sketch => {
                push_pdf_number(output, source_stroke_width / 2.0, "PDF fill weight")?;
                output.push_str(" w\n1 J\n/CS1 CS\n");
                push_color(output, fill_color.rgb_f64(), "SCN")?;
                push_rough_path(output, &fill.path)?;
                output.push_str("S\n");
            }
        }
        output.push_str("Q\n");
    }
    let stroke_opacity = opacity * stroke_color.alpha();
    if stroke_opacity != 1.0 {
        push_opacity_resource(output, opacity_resources, stroke_opacity)?;
    }
    push_pdf_number(output, stroke_width, "PDF stroke width")?;
    output.push_str(" w\n1 J\n/CS1 CS\n");
    match stroke_style {
        StrokeStyle::Solid => output.push_str("[] 0 d\n"),
        StrokeStyle::Dashed => {
            output.push_str("[8 ");
            push_pdf_number(output, 8.0 + source_stroke_width, "PDF dash gap")?;
            output.push_str("] 0 d\n");
        }
        StrokeStyle::Dotted => {
            output.push_str("[1.5 ");
            push_pdf_number(output, 6.0 + source_stroke_width, "PDF dot gap")?;
            output.push_str("] 0 d\n");
        }
    }
    push_color(output, stroke_color.rgb_f64(), "SCN")?;
    push_rough_path(output, &drawing.outline)?;
    output.push_str("S\nQ\n");
    Ok(())
}

fn push_rough_path(output: &mut String, path: &Path) -> Result<(), FormatEncodeError> {
    for operation in &path.operations {
        match operation {
            PathOperation::Move(point) => {
                push_rough_number(output, point.x)?;
                output.push(' ');
                push_rough_number(output, point.y)?;
                output.push_str(" m\n");
            }
            PathOperation::Line(point) => {
                push_rough_number(output, point.x)?;
                output.push(' ');
                push_rough_number(output, point.y)?;
                output.push_str(" l\n");
            }
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                for (index, value) in [
                    control_1.x,
                    control_1.y,
                    control_2.x,
                    control_2.y,
                    end.x,
                    end.y,
                ]
                .into_iter()
                .enumerate()
                {
                    if index != 0 {
                        output.push(' ');
                    }
                    push_rough_number(output, value)?;
                }
                output.push_str(" c\n");
            }
        }
    }
    Ok(())
}

fn push_freedraw_path(
    output: &mut String,
    path: &freedraw::ProjectedPath,
) -> Result<(), FormatEncodeError> {
    let mut current = None;
    for operation in &path.operations {
        match *operation {
            freedraw::PathOperation::Move(point) => {
                push_pdf_number(output, point.x, "PDF freedraw x")?;
                output.push(' ');
                push_pdf_number(output, point.y, "PDF freedraw y")?;
                output.push_str(" m\n");
                current = Some(point);
            }
            freedraw::PathOperation::Quadratic { control, end } => {
                let start = current.ok_or_else(|| {
                    FormatEncodeError::Encoding(
                        "PDF freedraw quadratic has no current point".to_owned(),
                    )
                })?;
                let control_1 = Point {
                    x: start.x + (control.x - start.x) * (2.0 / 3.0),
                    y: start.y + (control.y - start.y) * (2.0 / 3.0),
                };
                let control_2 = Point {
                    x: end.x + (control.x - end.x) * (2.0 / 3.0),
                    y: end.y + (control.y - end.y) * (2.0 / 3.0),
                };
                for (index, value) in [
                    control_1.x,
                    control_1.y,
                    control_2.x,
                    control_2.y,
                    end.x,
                    end.y,
                ]
                .into_iter()
                .enumerate()
                {
                    if index != 0 {
                        output.push(' ');
                    }
                    push_pdf_number(output, value, "PDF freedraw coordinate")?;
                }
                output.push_str(" c\n");
                current = Some(end);
            }
            freedraw::PathOperation::Line(point) => {
                push_pdf_number(output, point.x, "PDF freedraw x")?;
                output.push(' ');
                push_pdf_number(output, point.y, "PDF freedraw y")?;
                output.push_str(" l\n");
                current = Some(point);
            }
            freedraw::PathOperation::Close => output.push_str("h\n"),
        }
    }
    Ok(())
}

/// The `cm` that places a resolved transform's local origin: rotation about
/// the element center, then translation, as the SVG `translate() rotate()` pair
/// composes.
fn push_transform_matrix(
    output: &mut String,
    transform: &ResolvedTransform,
) -> Result<(), FormatEncodeError> {
    let (sin, cos) = if transform.angle_radians == 0.0 {
        (0.0, 1.0)
    } else {
        sin_cos(transform.angle_radians)
    };
    let offset_x = transform.translation_x + transform.center_x - cos * transform.center_x
        + sin * transform.center_y;
    let offset_y = transform.translation_y + transform.center_y
        - sin * transform.center_x
        - cos * transform.center_y;
    push_matrix(output, cos, sin, -sin, cos, offset_x, offset_y)
}

fn push_matrix(
    output: &mut String,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
) -> Result<(), FormatEncodeError> {
    for (index, value) in [a, b, c, d, e, f].into_iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        push_pdf_number(output, value, "PDF transformation component")?;
    }
    output.push_str(" cm\n");
    Ok(())
}

fn push_pdf_rectangle(
    output: &mut String,
    width: f64,
    height: f64,
) -> Result<(), FormatEncodeError> {
    output.push_str("0 0 ");
    push_pdf_number(output, width, "PDF fallback rectangle width")?;
    output.push(' ');
    push_pdf_number(output, height, "PDF fallback rectangle height")?;
    output.push_str(" re\n");
    Ok(())
}

fn push_fallback_body(
    output: &mut String,
    width: f64,
    height: f64,
    label: Option<&ResolvedLabel>,
    colors: FallbackColors,
    mappings: &[PdfGlyphMapping],
) -> Result<(), FormatEncodeError> {
    push_pdf_rectangle(output, width, height)?;
    output.push_str("W\nn\n/CS1 cs\n");
    push_color(output, colors.fill.rgb_f64(), "scn")?;
    push_pdf_rectangle(output, width, height)?;
    output.push_str("f\n");
    if width >= 2.0 && height >= 2.0 {
        output.push_str("/CS1 CS\n");
        push_color(output, colors.stroke.rgb_f64(), "SCN")?;
        output.push_str("2 w\n0 J\n0 j\n[6 4] 0 d\n");
        push_pdf_rectangle(output, width, height)?;
        output.push_str("S\n[] 0 d\n0 0 m\n");
        push_pdf_number(output, width, "PDF fallback cross width")?;
        output.push(' ');
        push_pdf_number(output, height, "PDF fallback cross height")?;
        output.push_str(" l\n");
        push_pdf_number(output, width, "PDF fallback reverse cross width")?;
        output.push_str(" 0 m\n0 ");
        push_pdf_number(output, height, "PDF fallback reverse cross height")?;
        output.push_str(" l\nS\n");
    }
    if let Some(label) = label {
        output.push_str("/Span << /ActualText <FEFF");
        push_utf16be_hex(output, &label.text)?;
        output.push_str("> >> BDC\n");
        push_pdf_label_text(output, label, colors.label, mappings)?;
        output.push_str("EMC\n");
    }
    Ok(())
}

fn push_pdf_label_text(
    output: &mut String,
    label: &ResolvedLabel,
    color: CanonicalColor,
    mappings: &[PdfGlyphMapping],
) -> Result<(), FormatEncodeError> {
    output.push_str("/CS1 cs\n");
    push_color(output, color.rgb_f64(), "scn")?;
    output.push_str("BT\n/F1 14 Tf\n");
    for glyph in &label.glyphs {
        output.push_str("1 0 0 -1 ");
        push_pdf_number(output, glyph.x, "PDF fallback glyph x")?;
        output.push(' ');
        push_pdf_number(output, glyph.y, "PDF fallback glyph y")?;
        output.push_str(" Tm\n<");
        let code = glyph_mapping_code(mappings, glyph)?;
        write!(output, "{code:04X}").map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF fallback glyph ID".to_owned())
        })?;
        output.push_str("> Tj\n");
    }
    output.push_str("ET\n");
    Ok(())
}

fn push_transformed_clip(
    output: &mut String,
    clip: &ResolvedClip,
) -> Result<(), FormatEncodeError> {
    let (sin, cos) = sin_cos(clip.transform.angle_radians);
    let offset_x = clip.transform.translation_x + clip.transform.center_x
        - cos * clip.transform.center_x
        + sin * clip.transform.center_y;
    let offset_y = clip.transform.translation_y + clip.transform.center_y
        - sin * clip.transform.center_x
        - cos * clip.transform.center_y;
    let map = |x: f64, y: f64| (cos * x - sin * y + offset_x, sin * x + cos * y + offset_y);
    push_mapped_clip(output, &clip.path, map, "PDF frame clip")
}

/// Emits `path` through `map` at full precision and installs it as the clip:
/// a frame clip mapped into page space before the element's `cm`, or an
/// image's corner clip left in element space after it.
fn push_mapped_clip(
    output: &mut String,
    path: &Path,
    map: impl Fn(f64, f64) -> (f64, f64),
    subject: &str,
) -> Result<(), FormatEncodeError> {
    for operation in &path.operations {
        match operation {
            PathOperation::Move(point) => {
                let (x, y) = map(point.x, point.y);
                push_pdf_point(output, x, y, "m")?;
            }
            PathOperation::Line(point) => {
                let (x, y) = map(point.x, point.y);
                push_pdf_point(output, x, y, "l")?;
            }
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                for point in [control_1, control_2, end] {
                    let (x, y) = map(point.x, point.y);
                    push_pdf_number(output, x, subject)?;
                    output.push(' ');
                    push_pdf_number(output, y, subject)?;
                    output.push(' ');
                }
                output.push_str("c\n");
            }
        }
    }
    output.push_str("W\nn\n");
    Ok(())
}

/// Emits an axis-aligned rectangle in the current space at full precision and
/// installs it as the clip.
fn push_rect_clip(
    output: &mut String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    for value in [x, y, width, height] {
        push_pdf_number(output, value, subject)?;
        output.push(' ');
    }
    output.push_str("re\nW\nn\n");
    Ok(())
}

fn push_utf16be_hex(output: &mut String, text: &str) -> Result<(), FormatEncodeError> {
    for unit in text.encode_utf16() {
        write!(output, "{unit:04X}").map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF fallback actual text".to_owned())
        })?;
    }
    Ok(())
}

fn push_pdf_point(
    output: &mut String,
    x: f64,
    y: f64,
    operator: &str,
) -> Result<(), FormatEncodeError> {
    push_pdf_number(output, x, "PDF fallback label x")?;
    output.push(' ');
    push_pdf_number(output, y, "PDF fallback label y")?;
    output.push(' ');
    output.push_str(operator);
    output.push('\n');
    Ok(())
}

fn push_opacity_resource(
    output: &mut String,
    resources: &[f64],
    opacity: f64,
) -> Result<(), FormatEncodeError> {
    let resource = resources
        .iter()
        .position(|candidate| candidate.to_bits() == opacity.to_bits())
        .ok_or_else(|| {
            FormatEncodeError::Encoding("PDF opacity resource has no owner".to_owned())
        })?;
    writeln!(output, "/GS{} gs", resource + 1)
        .map_err(|_| FormatEncodeError::Encoding("failed to write PDF opacity resource".to_owned()))
}

fn push_color(
    output: &mut String,
    color: [f64; 3],
    operator: &str,
) -> Result<(), FormatEncodeError> {
    for (index, value) in color.into_iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        push_pdf_number(output, value, "PDF sRGB color component")?;
    }
    writeln!(output, " {operator}")
        .map_err(|_| FormatEncodeError::Encoding("failed to write PDF color".to_owned()))
}

fn push_pdf_number(
    output: &mut String,
    value: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    output.push_str(&format_pdf_number(value, subject)?);
    Ok(())
}
