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
    fallback_font_woff2: Option<Vec<u8>>,
    mandatory_manifest: Option<String>,
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
    ) -> Result<Self, FormatEncodeError> {
        let scale = match request.sizing() {
            Sizing::Scale(scale) => scale,
            Sizing::MaxDimension(_) => 1.0,
        };
        let fallback_font_woff2 = scene.fallback_font_woff2;
        let mandatory_manifest = scene.mandatory_manifest;
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
                    elements.push(SvgElement::RoughOutline {
                        source: outline.source,
                        drawing: outline.drawing,
                        frame_clips: outline.frame_clips,
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
                        opacity: outline.opacity,
                        background_color: outline.background_color,
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
                    });
                    fallback_index = fallback_index.saturating_add(1);
                }
                ResolvedElement::Fallback(fallback) => {
                    elements.push(SvgElement::Fallback {
                        source: fallback.source,
                        clip_index: fallback_index,
                        label: fallback.label.map(|label| label.text),
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
            fallback_font_woff2,
            mandatory_manifest,
            elements,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let mut capacity = 256_usize;
        if let Some(font) = &self.fallback_font_woff2 {
            capacity = checked_capacity_add(
                capacity,
                font.len()
                    .checked_add(2)
                    .and_then(|length| length.checked_div(3))
                    .and_then(|groups| groups.checked_mul(4))
                    .and_then(|encoded| encoded.checked_add(256))
                    .ok_or_else(svg_size_error)?,
            )?;
        }
        if let Some(manifest) = &self.mandatory_manifest {
            capacity = checked_capacity_add(capacity, escaped_capacity(manifest)?)?;
            capacity = checked_capacity_add(capacity, 256)?;
        }
        capacity = checked_capacity_add(capacity, usize::from(self.background.is_some()) * 32)?;
        for element in &self.elements {
            match element {
                SvgElement::RoughOutline { drawing, .. } => {
                    capacity = checked_capacity_add(
                        capacity,
                        drawing
                            .operation_count()
                            .checked_mul(200)
                            .ok_or_else(svg_size_error)?,
                    )?;
                    capacity = checked_capacity_add(capacity, 512)?;
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
                SvgElement::Fallback { label, link, .. } => {
                    if let Some(label) = label {
                        capacity = checked_capacity_add(capacity, escaped_capacity(label)?)?;
                    }
                    if let Some(link) = link {
                        capacity = checked_capacity_add(capacity, escaped_capacity(link)?)?;
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
        if let Some(manifest) = &self.mandatory_manifest {
            svg.push_str(concat!(
                "<!-- svg-source:excalidraw --><metadata>",
                "<excalidraw-render:manifest xmlns:excalidraw-render=\"",
                "urn:excalidraw-render:artifact-manifest:1\">"
            ));
            push_escaped_text(&mut svg, manifest);
            svg.push_str("</excalidraw-render:manifest></metadata>");
        }
        if let Some(font) = &self.fallback_font_woff2 {
            svg.push_str(concat!(
                "<defs><style>@font-face{font-family:'Liberation Sans';font-style:normal;",
                "font-weight:400;src:url(data:font/woff2;base64,"
            ));
            push_base64(&mut svg, font)?;
            svg.push_str(") format('woff2')}</style></defs>");
        }
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
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG plan outline lost its source identity".to_owned(),
                        ));
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
                    svg.push_str("</g>");
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
                        svg.push_str("\" x=\"0\" y=\"-8\" font-family=\"Liberation Sans\" font-size=\"14\" fill=\"");
                        svg.push_str(if *magic { "#7affd7" } else { "#1b1b1f" });
                        svg.push_str("\">");
                        push_escaped_text(&mut svg, &name.text);
                        svg.push_str("</text>");
                    }
                    svg.push_str("</g>");
                }
                SvgElement::Fallback {
                    source,
                    clip_index,
                    label,
                    link,
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
                    if let Some(link) = link {
                        svg.push_str("<a href=\"");
                        push_escaped_attribute(&mut svg, link);
                        svg.push_str("\">");
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
                            "\" font-family=\"Liberation Sans\" font-size=\"14\" ",
                            "text-anchor=\"middle\" dominant-baseline=\"central\">"
                        ));
                        push_escaped_text(&mut svg, label);
                        svg.push_str("</text>");
                    }
                    svg.push_str("</g></g>");
                    if link.is_some() {
                        svg.push_str("</a>");
                    }
                }
            }
            for _ in 0..frame_clip_count {
                svg.push_str("</g>");
            }
        }
        svg.push_str("</svg>");
        Ok(svg.into_bytes())
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
        self.elements.get(index).map(|element| match element {
            SvgElement::RoughOutline { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Freedraw { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Frame { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Fallback { source, .. } => (source.id.as_deref(), source.pointer.as_str()),
        })
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
