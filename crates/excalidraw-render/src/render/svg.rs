use std::fmt::Write as _;

use crate::{color::CanonicalColor, RenderRequest, Sizing};

use super::{
    rough::{Drawing, FillKind, Path, PathOperation},
    FormatEncodeError, NormalizedElementSource, ResolvedElement, ResolvedScene, StrokeStyle,
};

pub(super) struct SvgPlan {
    logical_width: f64,
    logical_height: f64,
    output_width: f64,
    output_height: f64,
    background: Option<CanonicalColor>,
    fallback_colors: super::FallbackColors,
    font_bundle: super::svg_fonts::Bundle,
    mandatory_notices: Option<super::notices::MandatoryNotices>,
    metadata: Option<super::metadata::ArtifactMetadata>,
    elements: Vec<SvgElement>,
}

enum SvgElement {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        frame_clips: Vec<super::ResolvedClip>,
        translation_x: f64,
        translation_y: f64,
        rotation_degrees: f64,
        center_x: f64,
        center_y: f64,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
        background_color: CanonicalColor,
        label_mask: Option<super::ResolvedLabelMask>,
        link: Option<String>,
        generated_label: Option<super::text::ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
        width: f64,
        height: f64,
    },
    Freedraw {
        source: NormalizedElementSource,
        path_data: String,
        background_fill: Option<super::rough::Fill>,
        frame_clips: Vec<super::ResolvedClip>,
        translation_x: f64,
        translation_y: f64,
        rotation_degrees: f64,
        center_x: f64,
        center_y: f64,
        color: CanonicalColor,
        background_color: Option<CanonicalColor>,
        source_stroke_width: f64,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Frame {
        source: NormalizedElementSource,
        name: Option<super::text::ResolvedLabel>,
        magic: bool,
        outline_path: Option<Path>,
        frame_clips: Vec<super::ResolvedClip>,
        translation_x: f64,
        translation_y: f64,
        rotation_degrees: f64,
        center_x: f64,
        center_y: f64,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Text {
        source: NormalizedElementSource,
        text: super::text::ResolvedText,
        frame_clips: Vec<super::ResolvedClip>,
        transform: super::ResolvedTransform,
        color: CanonicalColor,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Image {
        source: NormalizedElementSource,
        asset: std::sync::Arc<super::ResolvedImageAsset>,
        frame_clips: Vec<super::ResolvedClip>,
        transform: super::ResolvedTransform,
        opacity: f64,
        source_rect: super::ResolvedImageSourceRect,
        scale: [i8; 2],
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Fallback {
        source: super::FallbackSource,
        clip_index: usize,
        label: Option<String>,
        link: Option<String>,
        frame_clips: Vec<super::ResolvedClip>,
        width: f64,
        height: f64,
        translation_x: f64,
        translation_y: f64,
        rotation_degrees: f64,
        center_x: f64,
        center_y: f64,
        opacity: f64,
    },
}

impl SvgElement {
    fn frame_clips(&self) -> &[super::ResolvedClip] {
        match self {
            Self::RoughOutline { frame_clips, .. }
            | Self::Freedraw { frame_clips, .. }
            | Self::Frame { frame_clips, .. }
            | Self::Text { frame_clips, .. }
            | Self::Image { frame_clips, .. }
            | Self::Fallback { frame_clips, .. } => frame_clips,
        }
    }
}

fn push_frame_clip_prefix(
    svg: &mut String,
    element_index: usize,
    legacy_fallback_index: Option<usize>,
    clips: &[super::ResolvedClip],
) -> Result<(), FormatEncodeError> {
    for (clip_index, clip) in clips.iter().enumerate() {
        svg.push_str("<clipPath id=\"");
        push_frame_clip_id(svg, element_index, legacy_fallback_index, clip_index)?;
        svg.push_str("\"><path d=\"");
        push_rough_path(svg, &clip.path)?;
        svg.push_str("\" transform=\"translate(");
        push_svg_number(
            svg,
            clip.transform.translation_x,
            "SVG frame clip translation x",
        )?;
        svg.push(' ');
        push_svg_number(
            svg,
            clip.transform.translation_y,
            "SVG frame clip translation y",
        )?;
        svg.push_str(") rotate(");
        push_svg_number(
            svg,
            180.0 * clip.transform.angle_radians / std::f64::consts::PI,
            "SVG frame clip rotation",
        )?;
        svg.push(' ');
        push_svg_number(svg, clip.transform.center_x, "SVG frame clip center x")?;
        svg.push(' ');
        push_svg_number(svg, clip.transform.center_y, "SVG frame clip center y")?;
        svg.push_str(")\"/></clipPath>");
    }
    for clip_index in 0..clips.len() {
        svg.push_str("<g clip-path=\"url(#");
        push_frame_clip_id(svg, element_index, legacy_fallback_index, clip_index)?;
        svg.push_str(")\">");
    }
    Ok(())
}

fn push_svg_link_hit_rect(
    svg: &mut String,
    link: Option<&str>,
    width: f64,
    height: f64,
) -> Result<(), FormatEncodeError> {
    if link.is_none() {
        return Ok(());
    }
    svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
    push_svg_number(svg, width, "SVG link hit width")?;
    svg.push_str("\" height=\"");
    push_svg_number(svg, height, "SVG link hit height")?;
    svg.push_str("\" fill=\"transparent\"/>");
    Ok(())
}

fn push_frame_clip_id(
    svg: &mut String,
    element_index: usize,
    legacy_fallback_index: Option<usize>,
    clip_index: usize,
) -> Result<(), FormatEncodeError> {
    if let Some(fallback_index) = legacy_fallback_index {
        write!(svg, "frame-fallback-{fallback_index}").map_err(|_| {
            FormatEncodeError::Encoding("failed to write SVG fallback clip ID".to_owned())
        })?;
        if clip_index != 0 {
            write!(svg, "-{clip_index}").map_err(|_| {
                FormatEncodeError::Encoding("failed to write SVG fallback clip suffix".to_owned())
            })?;
        }
    } else {
        write!(svg, "frame-clip-{element_index}-{clip_index}").map_err(|_| {
            FormatEncodeError::Encoding("failed to write SVG frame clip ID".to_owned())
        })?;
    }
    Ok(())
}

impl SvgPlan {
    pub(super) fn new(
        scene: ResolvedScene,
        request: &RenderRequest,
        mut font_bundle: super::svg_fonts::Bundle,
    ) -> Result<Self, FormatEncodeError> {
        let scale = match request.sizing() {
            Sizing::Scale(scale) => scale,
            Sizing::MaxDimension(_) => 1.0,
        };
        let mandatory_notices = font_bundle.take_mandatory_notices();
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(scene.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "svg_elements",
            })?;
        let mut fallback_index = 0_usize;
        for element in scene.elements {
            match element {
                ResolvedElement::Outline(outline) => {
                    if outline.source.id().is_empty() {
                        return Err(FormatEncodeError::Encoding(format!(
                            "outline at {} has no normalized identity",
                            outline.source.pointer()
                        )));
                    }
                    let frame_clips = if outline.generated_label.is_some() {
                        Vec::new()
                    } else {
                        outline.frame_clips
                    };
                    elements.push(SvgElement::RoughOutline {
                        source: outline.source,
                        drawing: outline.drawing,
                        frame_clips,
                        translation_x: outline.transform.translation_x,
                        translation_y: outline.transform.translation_y,
                        rotation_degrees: 180.0 * outline.transform.angle_radians
                            / std::f64::consts::PI,
                        center_x: outline.transform.center_x,
                        center_y: outline.transform.center_y,
                        stroke_color: outline.stroke_color,
                        fill_color: outline.fill_color,
                        stroke_width: outline.stroke_width,
                        stroke_style: outline.stroke_style,
                        source_stroke_width: outline.source_stroke_width,
                        opacity: if outline.generated_label.is_some() {
                            outline.element_opacity
                        } else {
                            outline.opacity
                        },
                        background_color: outline.background_color,
                        label_mask: outline.label_mask,
                        link: outline.link,
                        generated_label: outline.generated_label,
                        generated_label_color: outline.generated_label_color,
                        generated_label_offset_y: outline.generated_label_offset_y,
                        width: outline.width,
                        height: outline.height,
                    });
                }
                ResolvedElement::Freedraw(freedraw) => {
                    elements.push(SvgElement::Freedraw {
                        source: freedraw.source,
                        path_data: freedraw_path_data(&freedraw.path)?,
                        background_fill: freedraw.background_fill,
                        frame_clips: freedraw.frame_clips,
                        translation_x: freedraw.transform.translation_x,
                        translation_y: freedraw.transform.translation_y,
                        rotation_degrees: 180.0 * freedraw.transform.angle_radians
                            / std::f64::consts::PI,
                        center_x: freedraw.transform.center_x,
                        center_y: freedraw.transform.center_y,
                        color: freedraw.color,
                        background_color: freedraw.background_color,
                        source_stroke_width: freedraw.source_stroke_width,
                        opacity: freedraw.opacity,
                        link: freedraw.link,
                        width: freedraw.width,
                        height: freedraw.height,
                    });
                }
                ResolvedElement::Frame(frame) => {
                    elements.push(SvgElement::Frame {
                        source: frame.source,
                        name: frame.name,
                        magic: frame.magic,
                        outline_path: frame.outline_path,
                        frame_clips: frame.frame_clips,
                        translation_x: frame.transform.translation_x,
                        translation_y: frame.transform.translation_y,
                        rotation_degrees: 180.0 * frame.transform.angle_radians
                            / std::f64::consts::PI,
                        center_x: frame.transform.center_x,
                        center_y: frame.transform.center_y,
                        opacity: frame.opacity,
                        link: frame.link,
                        width: frame.width,
                        height: frame.height,
                    });
                    fallback_index = fallback_index.saturating_add(1);
                }
                ResolvedElement::Text(text) => {
                    elements.push(SvgElement::Text {
                        source: text.source,
                        text: text.text,
                        frame_clips: text.frame_clips,
                        transform: text.transform,
                        color: text.color,
                        opacity: text.opacity,
                        link: text.link,
                        width: text.width,
                        height: text.height,
                    });
                }
                ResolvedElement::Image(image) => {
                    elements.push(SvgElement::Image {
                        source: image.source,
                        asset: image.asset,
                        frame_clips: image.frame_clips,
                        transform: image.transform,
                        opacity: image.opacity,
                        source_rect: image.source_rect,
                        scale: image.scale,
                        link: image.link,
                        width: image.width,
                        height: image.height,
                    });
                }
                ResolvedElement::ImageFallback(_) => {}
                ResolvedElement::Fallback(fallback) => {
                    elements.push(SvgElement::Fallback {
                        source: fallback.source,
                        clip_index: fallback_index,
                        label: fallback.label.map(|label| label.display_text),
                        link: fallback.link,
                        frame_clips: fallback.frame_clips,
                        width: fallback.width,
                        height: fallback.height,
                        translation_x: fallback.transform.translation_x,
                        translation_y: fallback.transform.translation_y,
                        rotation_degrees: 180.0 * fallback.transform.angle_radians
                            / std::f64::consts::PI,
                        center_x: fallback.transform.center_x,
                        center_y: fallback.transform.center_y,
                        opacity: fallback.opacity,
                    });
                    fallback_index = fallback_index.saturating_add(1);
                }
            }
        }
        Ok(Self {
            logical_width: scene.logical_width,
            logical_height: scene.logical_height,
            output_width: scene.logical_width * scale,
            output_height: scene.logical_height * scale,
            background: scene.include_background.then_some(scene.background_color),
            fallback_colors: scene.fallback_colors,
            font_bundle,
            mandatory_notices,
            metadata: None,
            elements,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let mut capacity = 256_usize;
        capacity = checked_capacity_add(
            capacity,
            self.font_bundle
                .encoded_capacity()
                .ok_or_else(svg_size_error)?,
        )?;
        if let Some(manifest) = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::manifest)
        {
            capacity = escaped_capacity(manifest)
                .and_then(|manifest_capacity| checked_capacity_add(capacity, manifest_capacity))
                .and_then(|capacity| checked_capacity_add(capacity, 256))
                .map_err(|_| {
                    super::metadata_embedding_error(
                        crate::DiagnosticCode::METADATA_MANIFEST_EMBEDDING_FAILED,
                        crate::MetadataComponent::Manifest,
                    )
                })?;
        }
        if let Some(wrapper) = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::scene_wrapper)
        {
            capacity = checked_capacity_add(
                capacity,
                wrapper
                    .len()
                    .saturating_add(2)
                    .div_ceil(3)
                    .saturating_mul(4),
            )
            .and_then(|capacity| checked_capacity_add(capacity, 256))
            .map_err(|_| {
                super::metadata_embedding_error(
                    crate::DiagnosticCode::METADATA_SCENE_EMBEDDING_FAILED,
                    crate::MetadataComponent::EmbeddedScene,
                )
            })?;
        }
        capacity = checked_capacity_add(capacity, usize::from(self.background.is_some()) * 32)?;
        for element in &self.elements {
            if let Some(link) = element.link() {
                capacity = checked_capacity_add(capacity, escaped_capacity(link)?)?;
                capacity = checked_capacity_add(capacity, 32)?;
            }
            match element {
                SvgElement::RoughOutline {
                    drawing,
                    generated_label,
                    ..
                } => {
                    capacity = checked_capacity_add(
                        capacity,
                        drawing
                            .operation_count()
                            .checked_mul(200)
                            .ok_or_else(svg_size_error)?,
                    )?;
                    capacity = checked_capacity_add(capacity, 512)?;
                    if let Some(label) = generated_label {
                        capacity = checked_capacity_add(
                            capacity,
                            label.lines.iter().fold(256_usize, |size, line| {
                                size.saturating_add(line.text.len()).saturating_add(192)
                            }),
                        )?;
                    }
                }
                SvgElement::Freedraw {
                    path_data,
                    background_fill,
                    ..
                } => {
                    capacity = checked_capacity_add(capacity, path_data.len())?;
                    if let Some(fill) = background_fill {
                        capacity = checked_capacity_add(
                            capacity,
                            fill.path
                                .operations
                                .len()
                                .checked_mul(200)
                                .ok_or_else(svg_size_error)?,
                        )?;
                    }
                    capacity = checked_capacity_add(capacity, 256)?;
                }
                SvgElement::Frame { name, .. } => {
                    if let Some(name) = name {
                        capacity = checked_capacity_add(capacity, escaped_capacity(&name.text)?)?;
                    }
                    capacity = checked_capacity_add(capacity, 512)?;
                }
                SvgElement::Text { text, .. } => {
                    for line in &text.lines {
                        capacity = checked_capacity_add(capacity, escaped_capacity(&line.text)?)?;
                    }
                    capacity =
                        checked_capacity_add(capacity, text.lines.len().saturating_mul(192))?;
                }
                SvgElement::Image { asset, .. } => {
                    let encoded = asset
                        .source_bytes
                        .len()
                        .checked_add(2)
                        .and_then(|length| length.checked_div(3))
                        .and_then(|groups| groups.checked_mul(4))
                        .and_then(|length| length.checked_add(512))
                        .ok_or_else(svg_size_error)?;
                    capacity = checked_capacity_add(capacity, encoded)?;
                }
                SvgElement::Fallback { label, .. } => {
                    if let Some(label) = label {
                        capacity = checked_capacity_add(capacity, escaped_capacity(label)?)?;
                    }
                    capacity = checked_capacity_add(capacity, 1_024)?;
                }
            }
        }
        let mut svg = String::new();
        svg.try_reserve_exact(capacity)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "artifact",
                subject: "svg_output",
            })?;
        svg.push_str("<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 ");
        push_svg_number(&mut svg, self.logical_width, "SVG width")?;
        svg.push(' ');
        push_svg_number(&mut svg, self.logical_height, "SVG height")?;
        svg.push_str("\" width=\"");
        push_svg_number(&mut svg, self.output_width, "scaled SVG width")?;
        svg.push_str("\" height=\"");
        push_svg_number(&mut svg, self.output_height, "scaled SVG height")?;
        svg.push_str("\">");
        let scene_wrapper = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::scene_wrapper);
        let manifest = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::manifest);
        svg.push_str("<!-- svg-source:excalidraw --><metadata>");
        if let Some(wrapper) = scene_wrapper {
            svg.push_str(concat!(
                "<!-- payload-type:application/vnd.excalidraw+json -->",
                "<!-- payload-version:2 --><!-- payload-start -->"
            ));
            push_base64(&mut svg, wrapper)?;
            svg.push_str("<!-- payload-end -->");
        }
        if let Some(manifest) = manifest {
            svg.push_str("<excalidraw-render:manifest xmlns:excalidraw-render=\"urn:excalidraw-render:artifact-manifest:1\">");
            push_escaped_text(&mut svg, manifest);
            svg.push_str("</excalidraw-render:manifest>");
        }
        svg.push_str("</metadata>");
        self.font_bundle.write(&mut svg, push_base64)?;
        if let Some(background) = &self.background {
            svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
            push_svg_number(&mut svg, self.logical_width, "SVG background width")?;
            svg.push_str("\" height=\"");
            push_svg_number(&mut svg, self.logical_height, "SVG background height")?;
            svg.push_str("\" fill=\"");
            background.write_hex(&mut svg).map_err(|_| {
                FormatEncodeError::Encoding("failed to write SVG background color".to_owned())
            })?;
            if background.alpha() != 1.0 {
                svg.push_str("\" fill-opacity=\"");
                push_svg_number(&mut svg, background.alpha(), "SVG background alpha")?;
            }
            svg.push_str("\"/>");
        }
        for (element_index, element) in self.elements.iter().enumerate() {
            let frame_clip_count = element.frame_clips().len();
            let legacy_fallback_index = match element {
                SvgElement::Fallback { clip_index, .. } => Some(*clip_index),
                _ => None,
            };
            push_frame_clip_prefix(
                &mut svg,
                element_index,
                legacy_fallback_index,
                element.frame_clips(),
            )?;
            if let Some(link) = element.link() {
                svg.push_str("<a href=\"");
                push_escaped_attribute(&mut svg, link);
                svg.push_str("\">");
            }
            match element {
                SvgElement::RoughOutline {
                    source,
                    drawing,
                    frame_clips: _,
                    translation_x,
                    translation_y,
                    rotation_degrees,
                    center_x,
                    center_y,
                    stroke_color,
                    fill_color,
                    stroke_width,
                    stroke_style,
                    source_stroke_width,
                    opacity,
                    background_color,
                    label_mask,
                    generated_label,
                    generated_label_color,
                    generated_label_offset_y,
                    link,
                    width,
                    height,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG plan outline lost its source identity".to_owned(),
                        ));
                    }
                    if let Some(mask) = label_mask {
                        svg.push_str("<defs><mask id=\"arrow-label-mask-");
                        write!(svg, "{element_index}").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG arrow label mask ID".to_owned(),
                            )
                        })?;
                        svg.push_str("\" maskUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"");
                        push_svg_number(&mut svg, self.logical_width, "SVG label mask width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, self.logical_height, "SVG label mask height")?;
                        svg.push_str(
                            "\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/><rect x=\"",
                        );
                        push_svg_number(&mut svg, mask.x, "SVG label mask x")?;
                        svg.push_str("\" y=\"");
                        push_svg_number(&mut svg, mask.y, "SVG label mask y")?;
                        svg.push_str("\" width=\"");
                        push_svg_number(&mut svg, mask.width, "SVG label mask rectangle width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, mask.height, "SVG label mask rectangle height")?;
                        svg.push_str(
                            "\" fill=\"black\"/></mask></defs><g mask=\"url(#arrow-label-mask-",
                        );
                        write!(svg, "{element_index}").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG arrow label mask reference".to_owned(),
                            )
                        })?;
                        svg.push_str(")\">");
                    }
                    svg.push_str("<g stroke-linecap=\"round\"");
                    svg.push_str(" transform=\"translate(");
                    push_svg_number(&mut svg, *translation_x, "SVG translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *translation_y, "SVG translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(&mut svg, *rotation_degrees, "SVG rotation")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_x, "SVG rotation center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_y, "SVG rotation center y")?;
                    svg.push_str(")\"");
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    push_svg_drawing(
                        &mut svg,
                        drawing,
                        *stroke_color,
                        *fill_color,
                        *stroke_width,
                        *stroke_style,
                        *source_stroke_width,
                        *opacity,
                    )?;
                    for decoration in &drawing.decorations {
                        let decoration_fill = super::decoration_fill_color(
                            decoration,
                            *stroke_color,
                            *background_color,
                        );
                        let (decoration_style, decoration_stroke_width) =
                            super::decoration_stroke_style(decoration, *source_stroke_width);
                        push_svg_drawing(
                            &mut svg,
                            &decoration.drawing,
                            *stroke_color,
                            Some(decoration_fill),
                            *stroke_width,
                            decoration_style,
                            decoration_stroke_width,
                            *opacity,
                        )?;
                    }
                    if let (Some(label), Some(color)) = (generated_label, generated_label_color) {
                        svg.push_str("<text data-source-text=\"");
                        for line in &label.lines {
                            push_escaped_attribute(&mut svg, &line.text);
                        }
                        svg.push_str("\" xml:space=\"preserve\" fill=\"");
                        color.write_hex(&mut svg).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG embeddable label color".to_owned(),
                            )
                        })?;
                        svg.push_str("\" font-family=\"ExcalidrawRender-9\" font-size=\"");
                        push_svg_number(&mut svg, label.font_size, "SVG embeddable font size")?;
                        svg.push_str("\">");
                        for line in &label.lines {
                            if !line.runs.is_empty() {
                                for run in &line.runs {
                                    let glyphs = line
                                        .glyphs
                                        .get(run.glyph_start..run.glyph_end)
                                        .ok_or_else(|| {
                                            FormatEncodeError::Encoding(
                                                "SVG embeddable run has invalid glyph bounds"
                                                    .to_owned(),
                                            )
                                        })?;
                                    svg.push_str("<tspan font-family=\"");
                                    push_escaped_attribute(
                                        &mut svg,
                                        super::svg_fonts::Bundle::effective_family(run.font_family),
                                    );
                                    svg.push_str(if run.rtl {
                                        "\" direction=\"rtl\" x=\""
                                    } else {
                                        "\" direction=\"ltr\" x=\""
                                    });
                                    for (index, glyph) in glyphs.iter().enumerate() {
                                        if index != 0 {
                                            svg.push(' ');
                                        }
                                        push_svg_number(
                                            &mut svg,
                                            glyph.x,
                                            "SVG embeddable run glyph x",
                                        )?;
                                    }
                                    svg.push_str("\" y=\"");
                                    for (index, glyph) in glyphs.iter().enumerate() {
                                        if index != 0 {
                                            svg.push(' ');
                                        }
                                        push_svg_number(
                                            &mut svg,
                                            glyph.y + generated_label_offset_y,
                                            "SVG embeddable run glyph y",
                                        )?;
                                    }
                                    svg.push_str("\">");
                                    push_escaped_text(&mut svg, &run.display_text);
                                    svg.push_str("</tspan>");
                                }
                                continue;
                            }
                            svg.push_str("<tspan x=\"");
                            for (index, glyph) in line.glyphs.iter().enumerate() {
                                if index != 0 {
                                    svg.push(' ');
                                }
                                push_svg_number(&mut svg, glyph.x, "SVG embeddable glyph x")?;
                            }
                            svg.push_str("\" y=\"");
                            if line.glyphs.is_empty() {
                                push_svg_number(
                                    &mut svg,
                                    line.baseline + generated_label_offset_y,
                                    "SVG embeddable baseline",
                                )?;
                            } else {
                                for (index, glyph) in line.glyphs.iter().enumerate() {
                                    if index != 0 {
                                        svg.push(' ');
                                    }
                                    push_svg_number(
                                        &mut svg,
                                        glyph.y + generated_label_offset_y,
                                        "SVG embeddable glyph y",
                                    )?;
                                }
                            }
                            svg.push_str("\">");
                            push_escaped_text(&mut svg, &line.display_text);
                            svg.push_str("</tspan>");
                        }
                        svg.push_str("</text>");
                    }
                    svg.push_str("</g>");
                    if label_mask.is_some() {
                        svg.push_str("</g>");
                    }
                }
                SvgElement::Freedraw {
                    source,
                    path_data,
                    background_fill,
                    frame_clips: _,
                    translation_x,
                    translation_y,
                    rotation_degrees,
                    center_x,
                    center_y,
                    color,
                    background_color,
                    source_stroke_width,
                    opacity,
                    link,
                    width,
                    height,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG plan freedraw lost its source identity".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, *translation_x, "SVG freedraw translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *translation_y, "SVG freedraw translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(&mut svg, *rotation_degrees, "SVG freedraw rotation")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_x, "SVG freedraw center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_y, "SVG freedraw center y")?;
                    svg.push_str(")\">");
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    if let (Some(fill), Some(background_color)) =
                        (background_fill, background_color)
                    {
                        svg.push_str("<path d=\"");
                        push_rough_path(&mut svg, &fill.path)?;
                        match fill.kind {
                            FillKind::Solid => {
                                svg.push_str("\" stroke=\"none\" fill=\"");
                                background_color.write_hex(&mut svg).map_err(|_| {
                                    FormatEncodeError::Encoding(
                                        "failed to write SVG freedraw background".to_owned(),
                                    )
                                })?;
                                svg.push_str("\" fill-rule=\"evenodd\"");
                                let fill_opacity = background_color.alpha() * opacity;
                                if fill_opacity != 1.0 {
                                    svg.push_str(" fill-opacity=\"");
                                    push_svg_number(
                                        &mut svg,
                                        fill_opacity,
                                        "SVG freedraw background opacity",
                                    )?;
                                    svg.push('"');
                                }
                            }
                            FillKind::Sketch => {
                                svg.push_str("\" stroke=\"");
                                background_color.write_hex(&mut svg).map_err(|_| {
                                    FormatEncodeError::Encoding(
                                        "failed to write SVG freedraw sketch fill".to_owned(),
                                    )
                                })?;
                                svg.push_str("\" stroke-width=\"");
                                push_svg_number(
                                    &mut svg,
                                    source_stroke_width / 2.0,
                                    "SVG freedraw fill weight",
                                )?;
                                svg.push_str("\" fill=\"none\"");
                                let fill_opacity = background_color.alpha() * opacity;
                                if fill_opacity != 1.0 {
                                    svg.push_str(" stroke-opacity=\"");
                                    push_svg_number(
                                        &mut svg,
                                        fill_opacity,
                                        "SVG freedraw sketch opacity",
                                    )?;
                                    svg.push('"');
                                }
                            }
                        }
                        svg.push_str("/> ");
                    }
                    svg.push_str("<path d=\"");
                    svg.push_str(path_data);
                    svg.push_str("\" fill=\"");
                    color.write_hex(&mut svg).map_err(|_| {
                        FormatEncodeError::Encoding("failed to write SVG freedraw color".to_owned())
                    })?;
                    let effective_opacity = color.alpha() * opacity;
                    if effective_opacity != 1.0 {
                        svg.push_str("\" fill-opacity=\"");
                        push_svg_number(
                            &mut svg,
                            effective_opacity,
                            "SVG freedraw effective opacity",
                        )?;
                    }
                    svg.push_str("\"/></g>");
                }
                SvgElement::Frame {
                    source,
                    name,
                    magic,
                    outline_path,
                    frame_clips: _,
                    translation_x,
                    translation_y,
                    rotation_degrees,
                    center_x,
                    center_y,
                    opacity,
                    link,
                    width,
                    height,
                    ..
                } => {
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, *translation_x, "SVG frame translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *translation_y, "SVG frame translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(&mut svg, *rotation_degrees, "SVG frame rotation")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_x, "SVG frame center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_y, "SVG frame center y")?;
                    svg.push_str(")\"");
                    if *opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG frame opacity")?;
                        svg.push('"');
                    }
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    if let Some(outline_path) = outline_path {
                        svg.push_str("<path data-frame-outline=\"");
                        push_escaped_attribute(&mut svg, source.id());
                        svg.push_str("\" d=\"");
                        push_rough_path(&mut svg, outline_path)?;
                        svg.push_str("\" fill=\"none\" stroke=\"#bbbbbb\" stroke-width=\"2\" stroke-linejoin=\"round\"/>");
                    }
                    if let Some(name) = name {
                        svg.push_str("<text data-frame-name=\"");
                        push_escaped_attribute(&mut svg, source.id());
                        svg.push_str("\" data-source-text=\"");
                        push_escaped_attribute(&mut svg, &name.text);
                        svg.push_str("\" x=\"");
                        if name.glyphs.is_empty() {
                            svg.push('0');
                        } else {
                            for (index, glyph) in name.glyphs.iter().enumerate() {
                                if index != 0 {
                                    svg.push(' ');
                                }
                                push_svg_number(&mut svg, glyph.x, "SVG frame name glyph x")?;
                            }
                        }
                        svg.push_str("\" y=\"");
                        if name.glyphs.is_empty() {
                            push_svg_number(
                                &mut svg,
                                name.baseline - 24.0,
                                "SVG frame name baseline",
                            )?;
                        } else {
                            for (index, glyph) in name.glyphs.iter().enumerate() {
                                if index != 0 {
                                    svg.push(' ');
                                }
                                push_svg_number(
                                    &mut svg,
                                    glyph.y - 24.0,
                                    "SVG frame name glyph y",
                                )?;
                            }
                        }
                        svg.push_str(
                            "\" font-family=\"ExcalidrawRender-9\" font-size=\"14\" fill=\"",
                        );
                        svg.push_str(if *magic { "#7affd7" } else { "#1b1b1f" });
                        svg.push_str("\">");
                        push_escaped_text(&mut svg, &name.display_text);
                        svg.push_str("</text>");
                    }
                    svg.push_str("</g>");
                }
                SvgElement::Text {
                    source,
                    text,
                    frame_clips: _,
                    transform,
                    color,
                    opacity,
                    link,
                    width,
                    height,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG text lost its source identity".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, transform.translation_x, "SVG text translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.translation_y, "SVG text translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(
                        &mut svg,
                        180.0 * transform.angle_radians / std::f64::consts::PI,
                        "SVG text rotation",
                    )?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_x, "SVG text center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_y, "SVG text center y")?;
                    svg.push_str(")\"");
                    let effective_opacity = opacity * color.alpha();
                    if effective_opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, effective_opacity, "SVG text opacity")?;
                        svg.push('"');
                    }
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    svg.push_str("<text data-source-text=\"");
                    for line in &text.lines {
                        push_escaped_attribute(&mut svg, &line.text);
                    }
                    svg.push_str("\" xml:space=\"preserve\" fill=\"");
                    color.write_hex(&mut svg).map_err(|_| {
                        FormatEncodeError::Encoding("failed to write SVG text color".to_owned())
                    })?;
                    svg.push_str("\" font-family=\"");
                    push_escaped_attribute(
                        &mut svg,
                        super::svg_fonts::Bundle::effective_family(text.font_family),
                    );
                    svg.push_str("\" font-size=\"");
                    push_svg_number(&mut svg, text.font_size, "SVG text font size")?;
                    let (line_x, text_anchor) = match text.text_align {
                        super::text::ResolvedTextAlign::Left => (0.0, "start"),
                        super::text::ResolvedTextAlign::Center => (*width / 2.0, "middle"),
                        super::text::ResolvedTextAlign::Right => (*width, "end"),
                    };
                    svg.push_str("\" text-anchor=\"");
                    svg.push_str(text_anchor);
                    svg.push_str("\">");
                    for line in &text.lines {
                        if line.runs.is_empty() {
                            svg.push_str("<tspan x=\"");
                            push_svg_number(&mut svg, line_x, "SVG text line anchor x")?;
                            svg.push_str("\" y=\"");
                            push_svg_number(&mut svg, line.baseline, "SVG text line baseline")?;
                            svg.push_str("\">");
                            push_escaped_text(&mut svg, &line.display_text);
                            svg.push_str("</tspan>");
                        } else {
                            for run in &line.runs {
                                let glyphs = line
                                    .glyphs
                                    .get(run.glyph_start..run.glyph_end)
                                    .ok_or_else(|| {
                                        FormatEncodeError::Encoding(
                                            "SVG text run has invalid glyph bounds".to_owned(),
                                        )
                                    })?;
                                svg.push_str("<tspan font-family=\"");
                                push_escaped_attribute(
                                    &mut svg,
                                    super::svg_fonts::Bundle::effective_family(run.font_family),
                                );
                                svg.push_str(if run.rtl {
                                    "\" direction=\"rtl\" text-anchor=\"start\" x=\""
                                } else {
                                    "\" direction=\"ltr\" text-anchor=\"start\" x=\""
                                });
                                for (index, glyph) in glyphs.iter().enumerate() {
                                    if index != 0 {
                                        svg.push(' ');
                                    }
                                    push_svg_number(&mut svg, glyph.x, "SVG text run glyph x")?;
                                }
                                svg.push_str("\" y=\"");
                                for (index, glyph) in glyphs.iter().enumerate() {
                                    if index != 0 {
                                        svg.push(' ');
                                    }
                                    push_svg_number(&mut svg, glyph.y, "SVG text run glyph y")?;
                                }
                                svg.push_str("\">");
                                push_escaped_text(&mut svg, &run.display_text);
                                svg.push_str("</tspan>");
                            }
                        }
                        if line.text.ends_with('\n') {
                            svg.push('\n');
                        }
                    }
                    svg.push_str("</text></g>");
                }
                SvgElement::Image {
                    source,
                    asset,
                    transform,
                    opacity,
                    link,
                    width,
                    height,
                    source_rect,
                    scale,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG image lost its source identity".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, transform.translation_x, "SVG image translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.translation_y, "SVG image translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(
                        &mut svg,
                        180.0 * transform.angle_radians / std::f64::consts::PI,
                        "SVG image rotation",
                    )?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_x, "SVG image center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_y, "SVG image center y")?;
                    svg.push_str(")\"");
                    if *opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG image opacity")?;
                        svg.push('"');
                    }
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    let transformed = *scale != [1, 1]
                        || source_rect.x != 0.0
                        || source_rect.y != 0.0
                        || source_rect.width != f64::from(asset.width)
                        || source_rect.height != f64::from(asset.height);
                    if *scale != [1, 1] {
                        svg.push_str("<g transform=\"translate(");
                        push_svg_number(
                            &mut svg,
                            if scale[0] < 0 { *width } else { 0.0 },
                            "SVG image flip translation x",
                        )?;
                        svg.push(' ');
                        push_svg_number(
                            &mut svg,
                            if scale[1] < 0 { *height } else { 0.0 },
                            "SVG image flip translation y",
                        )?;
                        svg.push_str(") scale(");
                        write!(svg, "{} {}", scale[0], scale[1]).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG image flip scale".to_owned(),
                            )
                        })?;
                        svg.push_str(")\">");
                    }
                    if transformed {
                        svg.push_str("<svg width=\"");
                        push_svg_number(&mut svg, *width, "SVG cropped image width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, *height, "SVG cropped image height")?;
                        svg.push_str("\" viewBox=\"");
                        push_svg_number(&mut svg, source_rect.x, "SVG image crop x")?;
                        svg.push(' ');
                        push_svg_number(&mut svg, source_rect.y, "SVG image crop y")?;
                        svg.push(' ');
                        push_svg_number(&mut svg, source_rect.width, "SVG image crop width")?;
                        svg.push(' ');
                        push_svg_number(&mut svg, source_rect.height, "SVG image crop height")?;
                        svg.push_str("\" preserveAspectRatio=\"none\" overflow=\"hidden\">");
                    }
                    svg.push_str("<image href=\"data:");
                    svg.push_str(asset.media_type);
                    svg.push_str(";base64,");
                    push_base64(&mut svg, &asset.source_bytes)?;
                    svg.push_str("\" width=\"");
                    push_svg_number(
                        &mut svg,
                        if transformed {
                            f64::from(asset.width)
                        } else {
                            *width
                        },
                        "SVG image width",
                    )?;
                    svg.push_str("\" height=\"");
                    push_svg_number(
                        &mut svg,
                        if transformed {
                            f64::from(asset.height)
                        } else {
                            *height
                        },
                        "SVG image height",
                    )?;
                    svg.push_str("\"/>");
                    if transformed {
                        svg.push_str("</svg>");
                    }
                    if *scale != [1, 1] {
                        svg.push_str("</g>");
                    }
                    svg.push_str("</g>");
                }
                SvgElement::Fallback {
                    source,
                    clip_index,
                    label,
                    link: _,
                    frame_clips: _,
                    width,
                    height,
                    translation_x,
                    translation_y,
                    rotation_degrees,
                    center_x,
                    center_y,
                    opacity,
                } => {
                    if source.pointer.is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG fallback lost its source location".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, *translation_x, "SVG fallback translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *translation_y, "SVG fallback translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(&mut svg, *rotation_degrees, "SVG fallback rotation")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_x, "SVG fallback rotation center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, *center_y, "SVG fallback rotation center y")?;
                    svg.push_str(")\"");
                    if *opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG fallback opacity")?;
                        svg.push('"');
                    }
                    svg.push_str("><clipPath id=\"fallback-");
                    write!(svg, "{clip_index}").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write SVG fallback clip identity".to_owned(),
                        )
                    })?;
                    svg.push_str("\"><rect x=\"0\" y=\"0\" width=\"");
                    push_svg_number(&mut svg, *width, "SVG fallback clip width")?;
                    svg.push_str("\" height=\"");
                    push_svg_number(&mut svg, *height, "SVG fallback clip height")?;
                    svg.push_str("\"/></clipPath><g clip-path=\"url(#fallback-");
                    write!(svg, "{clip_index}").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write SVG fallback clip reference".to_owned(),
                        )
                    })?;
                    svg.push_str(")\"><rect x=\"0\" y=\"0\" width=\"");
                    push_svg_number(&mut svg, *width, "SVG fallback fill width")?;
                    svg.push_str("\" height=\"");
                    push_svg_number(&mut svg, *height, "SVG fallback fill height")?;
                    svg.push_str("\" fill=\"");
                    self.fallback_colors.fill.write_hex(&mut svg).map_err(|_| {
                        FormatEncodeError::Encoding("failed to write SVG fallback fill".to_owned())
                    })?;
                    svg.push_str("\"/>");
                    if *width >= 2.0 && *height >= 2.0 {
                        svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
                        push_svg_number(&mut svg, *width, "SVG fallback stroke width extent")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, *height, "SVG fallback stroke height extent")?;
                        svg.push_str("\" fill=\"none\" stroke=\"");
                        self.fallback_colors
                            .stroke
                            .write_hex(&mut svg)
                            .map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write SVG fallback stroke".to_owned(),
                                )
                            })?;
                        svg.push_str(concat!(
                            "\" stroke-width=\"2\" stroke-dasharray=\"6 4\" ",
                            "stroke-dashoffset=\"0\"/><path d=\"M0 0 L"
                        ));
                        push_svg_number(&mut svg, *width, "SVG fallback cross width")?;
                        svg.push(' ');
                        push_svg_number(&mut svg, *height, "SVG fallback cross height")?;
                        svg.push_str(" M");
                        push_svg_number(&mut svg, *width, "SVG fallback reverse cross width")?;
                        svg.push_str(" 0 L0 ");
                        push_svg_number(&mut svg, *height, "SVG fallback reverse cross height")?;
                        svg.push_str("\" fill=\"none\" stroke=\"");
                        self.fallback_colors
                            .stroke
                            .write_hex(&mut svg)
                            .map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write SVG fallback cross".to_owned(),
                                )
                            })?;
                        svg.push_str("\" stroke-width=\"2\"/>");
                    }
                    if let Some(label) = label {
                        svg.push_str("<text x=\"");
                        push_svg_number(&mut svg, *width / 2.0, "SVG fallback label x")?;
                        svg.push_str("\" y=\"");
                        push_svg_number(&mut svg, *height / 2.0, "SVG fallback label y")?;
                        svg.push_str("\" fill=\"");
                        self.fallback_colors
                            .label
                            .write_hex(&mut svg)
                            .map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write SVG fallback label color".to_owned(),
                                )
                            })?;
                        svg.push_str(concat!(
                            "\" font-family=\"ExcalidrawRender-9\" font-size=\"14\" ",
                            "text-anchor=\"middle\" dominant-baseline=\"central\">"
                        ));
                        push_escaped_text(&mut svg, label);
                        svg.push_str("</text>");
                    }
                    svg.push_str("</g></g>");
                }
            }
            if element.link().is_some() {
                svg.push_str("</a>");
            }
            for _ in 0..frame_clip_count {
                svg.push_str("</g>");
            }
        }
        svg.push_str("</svg>");
        Ok(svg.into_bytes())
    }

    pub(super) fn set_metadata(&mut self, metadata: super::metadata::ArtifactMetadata) {
        self.metadata = Some(metadata);
        self.mandatory_notices = None;
    }

    pub(super) fn take_mandatory_notices(&mut self) -> Option<super::notices::MandatoryNotices> {
        self.mandatory_notices.take()
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
        self.elements.get(index).map(|element| match element {
            SvgElement::RoughOutline { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Freedraw { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Frame { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Text { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Image { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Fallback { source, .. } => (source.id.as_deref(), source.pointer.as_str()),
        })
    }
}

impl SvgElement {
    fn link(&self) -> Option<&str> {
        match self {
            Self::RoughOutline { link, .. }
            | Self::Freedraw { link, .. }
            | Self::Frame { link, .. }
            | Self::Text { link, .. }
            | Self::Image { link, .. }
            | Self::Fallback { link, .. } => link.as_deref(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_svg_drawing(
    output: &mut String,
    drawing: &Drawing,
    stroke_color: CanonicalColor,
    fill_color: Option<CanonicalColor>,
    stroke_width: f64,
    stroke_style: StrokeStyle,
    source_stroke_width: f64,
    opacity: f64,
) -> Result<(), FormatEncodeError> {
    if let (Some(fill), Some(fill_color)) = (&drawing.fill, fill_color) {
        output.push_str("<path d=\"");
        push_rough_path(output, &fill.path)?;
        match fill.kind {
            FillKind::Solid => {
                output.push_str("\" stroke=\"none\" fill=\"");
                fill_color.write_hex(output).map_err(|_| {
                    FormatEncodeError::Encoding("failed to write SVG fill color".to_owned())
                })?;
                output.push('"');
                if drawing.shape.svg_uses_even_odd_fill() {
                    output.push_str(" fill-rule=\"evenodd\"");
                }
                if fill_color.alpha() * opacity != 1.0 {
                    output.push_str(" fill-opacity=\"");
                    push_svg_number(
                        output,
                        fill_color.alpha() * opacity,
                        "SVG effective fill alpha",
                    )?;
                    output.push('"');
                }
            }
            FillKind::Sketch => {
                output.push_str("\" stroke=\"");
                fill_color.write_hex(output).map_err(|_| {
                    FormatEncodeError::Encoding("failed to write SVG sketch color".to_owned())
                })?;
                output.push_str("\" stroke-width=\"");
                push_svg_number(output, source_stroke_width / 2.0, "SVG fill weight")?;
                output.push_str("\" fill=\"none\"");
                if fill_color.alpha() * opacity != 1.0 {
                    output.push_str(" stroke-opacity=\"");
                    push_svg_number(
                        output,
                        fill_color.alpha() * opacity,
                        "SVG effective fill alpha",
                    )?;
                    output.push('"');
                }
            }
        }
        output.push_str("/> ");
    }
    output.push_str("<path d=\"");
    push_rough_path(output, &drawing.outline)?;
    output.push_str("\" stroke=\"");
    stroke_color
        .write_hex(output)
        .map_err(|_| FormatEncodeError::Encoding("failed to write SVG stroke color".to_owned()))?;
    output.push_str("\" stroke-width=\"");
    push_svg_number(output, stroke_width, "SVG stroke width")?;
    output.push_str("\" fill=\"none\"");
    match stroke_style {
        StrokeStyle::Solid => {}
        StrokeStyle::Dashed => {
            output.push_str(" stroke-dasharray=\"8 ");
            push_svg_number(output, 8.0 + source_stroke_width, "SVG dash gap")?;
            output.push('"');
        }
        StrokeStyle::Dotted => {
            output.push_str(" stroke-dasharray=\"1.5 ");
            push_svg_number(output, 6.0 + source_stroke_width, "SVG dot gap")?;
            output.push('"');
        }
    }
    if stroke_color.alpha() * opacity != 1.0 {
        output.push_str(" stroke-opacity=\"");
        push_svg_number(
            output,
            stroke_color.alpha() * opacity,
            "SVG effective stroke alpha",
        )?;
        output.push('"');
    }
    output.push_str("/>");
    Ok(())
}

fn push_rough_path(output: &mut String, path: &Path) -> Result<(), FormatEncodeError> {
    for (index, operation) in path.operations.iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        match operation {
            PathOperation::Move(point) => {
                output.push('M');
                super::rough::push_number(output, point.x)?;
                output.push(' ');
                super::rough::push_number(output, point.y)?;
            }
            PathOperation::Line(point) => {
                output.push('L');
                super::rough::push_number(output, point.x)?;
                output.push(' ');
                super::rough::push_number(output, point.y)?;
            }
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                output.push('C');
                super::rough::push_number(output, control_1.x)?;
                output.push(' ');
                super::rough::push_number(output, control_1.y)?;
                output.push_str(", ");
                super::rough::push_number(output, control_2.x)?;
                output.push(' ');
                super::rough::push_number(output, control_2.y)?;
                output.push_str(", ");
                super::rough::push_number(output, end.x)?;
                output.push(' ');
                super::rough::push_number(output, end.y)?;
            }
        }
    }
    Ok(())
}

fn freedraw_path_data(path: &super::freedraw::Path) -> Result<String, FormatEncodeError> {
    let mut output = String::new();
    output
        .try_reserve(path.operations.len().saturating_mul(64))
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_plan",
            subject: "svg_freedraw_path_data",
        })?;
    for operation in &path.operations {
        match *operation {
            super::freedraw::PathOperation::Move(point) => {
                output.push_str("M ");
                push_freedraw_point(&mut output, point)?;
                output.push_str(" Q");
            }
            super::freedraw::PathOperation::Quadratic { control, end } => {
                output.push(' ');
                push_freedraw_point(&mut output, control)?;
                output.push(' ');
                push_freedraw_point(&mut output, end)?;
            }
            super::freedraw::PathOperation::Line(point) => {
                output.push_str(" L ");
                push_freedraw_point(&mut output, point)?;
            }
            super::freedraw::PathOperation::Close => output.push_str(" Z"),
        }
    }
    Ok(output)
}

fn push_freedraw_point(
    output: &mut String,
    point: super::rough::Point,
) -> Result<(), FormatEncodeError> {
    push_freedraw_number(output, point.x)?;
    output.push(',');
    push_freedraw_number(output, point.y)
}

fn push_freedraw_number(output: &mut String, value: f64) -> Result<(), FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(
            "SVG freedraw path contains a non-finite coordinate".to_owned(),
        ));
    }
    let token = super::freedraw::ecmascript_number(value);
    let Some(decimal) = token.find('.') else {
        output.push_str(&token);
        return Ok(());
    };
    let digits = token[decimal + 1..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count()
        .min(2);
    let retained_end = decimal + 1 + digits;
    let consumed_end = token[retained_end..]
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || matches!(byte, b'e' | b'-'))
        .count()
        + retained_end;
    output.push_str(&token[..retained_end]);
    output.push_str(&token[consumed_end..]);
    Ok(())
}

fn push_svg_number(
    output: &mut String,
    value: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(format!(
            "{subject} must be finite"
        )));
    }
    let value = if value == 0.0 { 0.0 } else { value };
    write!(output, "{value}")
        .map_err(|_| FormatEncodeError::Encoding(format!("failed to write {subject}")))
}

fn push_escaped_attribute(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '"' => output.push_str("&quot;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\n' => output.push_str("&#10;"),
            '\r' => output.push_str("&#13;"),
            '\t' => output.push_str("&#9;"),
            character => output.push(character),
        }
    }
}

fn push_escaped_text(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            character => output.push(character),
        }
    }
}

fn push_base64(output: &mut String, bytes: &[u8]) -> Result<(), FormatEncodeError> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(ALPHABET[usize::from(first >> 2)]));
        output.push(char::from(
            ALPHABET[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        if chunk.len() > 1 {
            output.push(char::from(
                ALPHABET[usize::from(((second & 0x0f) << 2) | (third >> 6))],
            ));
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(char::from(ALPHABET[usize::from(third & 0x3f)]));
        } else {
            output.push('=');
        }
    }
    Ok(())
}

fn escaped_capacity(value: &str) -> Result<usize, FormatEncodeError> {
    value.len().checked_mul(6).ok_or_else(svg_size_error)
}

fn checked_capacity_add(current: usize, added: usize) -> Result<usize, FormatEncodeError> {
    current.checked_add(added).ok_or_else(svg_size_error)
}

fn svg_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("SVG plan exceeds serializer capacity".to_owned())
}
