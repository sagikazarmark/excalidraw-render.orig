use std::fmt::Write as _;

use crate::{color::CanonicalColor, RenderRequest, Sizing};

use super::{
    rough::{Path, PathOperation},
    FormatEncodeError, NormalizedElementSource, ResolvedElement, ResolvedScene,
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
    RoughRectangle {
        source: NormalizedElementSource,
        path: Path,
        translation_x: f64,
        translation_y: f64,
        rotation_degrees: f64,
        center_x: f64,
        center_y: f64,
        stroke_color: CanonicalColor,
        stroke_width: f64,
        opacity: f64,
    },
    Fallback {
        source: super::FallbackSource,
        clip_index: usize,
        label: Option<String>,
        link: Option<String>,
        frame_clip: Option<super::ResolvedClip>,
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
                ResolvedElement::Rectangle(rectangle) => {
                    if rectangle.source.id().is_empty() {
                        return Err(FormatEncodeError::Encoding(format!(
                            "rectangle at {} has no normalized identity",
                            rectangle.source.pointer()
                        )));
                    }
                    elements.push(SvgElement::RoughRectangle {
                        source: rectangle.source,
                        path: rectangle.path,
                        translation_x: rectangle.transform.translation_x,
                        translation_y: rectangle.transform.translation_y,
                        rotation_degrees: 180.0 * rectangle.transform.angle_radians
                            / std::f64::consts::PI,
                        center_x: rectangle.transform.center_x,
                        center_y: rectangle.transform.center_y,
                        stroke_color: rectangle.stroke_color,
                        stroke_width: rectangle.stroke_width,
                        opacity: rectangle.opacity,
                    });
                }
                ResolvedElement::Fallback(fallback) => {
                    elements.push(SvgElement::Fallback {
                        source: fallback.source,
                        clip_index: fallback_index,
                        label: fallback.label.map(|label| label.text),
                        link: fallback.link,
                        frame_clip: fallback.frame_clip,
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
                SvgElement::RoughRectangle { path, .. } => {
                    capacity = checked_capacity_add(
                        capacity,
                        path.operations
                            .len()
                            .checked_mul(200)
                            .ok_or_else(svg_size_error)?,
                    )?;
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
        for element in &self.elements {
            match element {
                SvgElement::RoughRectangle {
                    source,
                    path,
                    translation_x,
                    translation_y,
                    rotation_degrees,
                    center_x,
                    center_y,
                    stroke_color,
                    stroke_width,
                    opacity,
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG plan rectangle lost its source identity".to_owned(),
                        ));
                    }
                    svg.push_str("<g stroke-linecap=\"round\" transform=\"translate(");
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
                    if *opacity != 1.0 {
                        svg.push_str(" stroke-opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG stroke opacity")?;
                        svg.push_str("\" fill-opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG fill opacity")?;
                        svg.push('"');
                    }
                    svg.push_str("><path d=\"");
                    push_rough_path(&mut svg, path)?;
                    svg.push_str("\" stroke=\"");
                    stroke_color.write_hex(&mut svg).map_err(|_| {
                        FormatEncodeError::Encoding("failed to write SVG stroke color".to_owned())
                    })?;
                    svg.push_str("\" stroke-width=\"");
                    push_svg_number(&mut svg, *stroke_width, "SVG stroke width")?;
                    svg.push_str("\" fill=\"none\"");
                    if stroke_color.alpha() != 1.0 {
                        svg.push_str(" stroke-opacity=\"");
                        push_svg_number(
                            &mut svg,
                            stroke_color.alpha() * opacity,
                            "SVG effective stroke alpha",
                        )?;
                        svg.push('"');
                    }
                    svg.push_str("/></g>");
                }
                SvgElement::Fallback {
                    source,
                    clip_index,
                    label,
                    link,
                    frame_clip,
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
                    if let Some(frame_clip) = frame_clip {
                        svg.push_str("<clipPath id=\"frame-fallback-");
                        write!(svg, "{clip_index}").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG frame clip identity".to_owned(),
                            )
                        })?;
                        svg.push_str("\"><rect x=\"0\" y=\"0\" width=\"");
                        push_svg_number(&mut svg, frame_clip.width, "SVG frame clip width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, frame_clip.height, "SVG frame clip height")?;
                        svg.push_str("\" transform=\"translate(");
                        push_svg_number(
                            &mut svg,
                            frame_clip.transform.translation_x,
                            "SVG frame clip translation x",
                        )?;
                        svg.push(' ');
                        push_svg_number(
                            &mut svg,
                            frame_clip.transform.translation_y,
                            "SVG frame clip translation y",
                        )?;
                        svg.push_str(") rotate(");
                        push_svg_number(
                            &mut svg,
                            180.0 * frame_clip.transform.angle_radians / std::f64::consts::PI,
                            "SVG frame clip rotation",
                        )?;
                        svg.push(' ');
                        push_svg_number(
                            &mut svg,
                            frame_clip.transform.center_x,
                            "SVG frame clip center x",
                        )?;
                        svg.push(' ');
                        push_svg_number(
                            &mut svg,
                            frame_clip.transform.center_y,
                            "SVG frame clip center y",
                        )?;
                        svg.push_str(")\"/></clipPath>");
                    }
                    if let Some(link) = link {
                        svg.push_str("<a href=\"");
                        push_escaped_attribute(&mut svg, link);
                        svg.push_str("\">");
                    }
                    if frame_clip.is_some() {
                        svg.push_str("<g clip-path=\"url(#frame-fallback-");
                        write!(svg, "{clip_index}").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG frame clip reference".to_owned(),
                            )
                        })?;
                        svg.push_str(")\">");
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
                    if frame_clip.is_some() {
                        svg.push_str("</g>");
                    }
                    if link.is_some() {
                        svg.push_str("</a>");
                    }
                }
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
            SvgElement::RoughRectangle { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Fallback { source, .. } => (source.id.as_deref(), source.pointer.as_str()),
        })
    }
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
