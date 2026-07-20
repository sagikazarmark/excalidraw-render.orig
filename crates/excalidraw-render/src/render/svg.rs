use std::fmt::Write as _;

use crate::{RenderRequest, Sizing};

use super::{
    rough::{Path, PathOperation},
    FormatEncodeError, ResolvedElement, ResolvedScene,
};

pub(super) struct SvgPlan {
    logical_width: f64,
    logical_height: f64,
    output_width: f64,
    output_height: f64,
    background: Option<String>,
    elements: Vec<SvgElement>,
}

enum SvgElement {
    RoughRectangle {
        source_id: String,
        source_pointer: &'static str,
        path: Path,
        translation_x: f64,
        translation_y: f64,
        rotation_degrees: f64,
        center_x: f64,
        center_y: f64,
        stroke_color: String,
        stroke_width: f64,
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
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(scene.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "svg_elements",
            })?;
        for element in scene.elements {
            match element {
                ResolvedElement::Rectangle(rectangle) => {
                    if rectangle.source_id.is_empty() {
                        return Err(FormatEncodeError::Encoding(format!(
                            "rectangle at {} has no normalized identity",
                            rectangle.source_pointer
                        )));
                    }
                    elements.push(SvgElement::RoughRectangle {
                        source_id: rectangle.source_id,
                        source_pointer: rectangle.source_pointer,
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
            }
        }
        Ok(Self {
            logical_width: scene.logical_width,
            logical_height: scene.logical_height,
            output_width: scene.logical_width * scale,
            output_height: scene.logical_height * scale,
            background: scene.include_background.then_some(scene.background_color),
            elements,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let mut capacity = 256_usize;
        if let Some(background) = &self.background {
            capacity = checked_capacity_add(capacity, escaped_capacity(background)?)?;
        }
        for element in &self.elements {
            match element {
                SvgElement::RoughRectangle {
                    path, stroke_color, ..
                } => {
                    capacity = checked_capacity_add(capacity, escaped_capacity(stroke_color)?)?;
                    capacity = checked_capacity_add(
                        capacity,
                        path.operations
                            .len()
                            .checked_mul(200)
                            .ok_or_else(svg_size_error)?,
                    )?;
                    capacity = checked_capacity_add(capacity, 512)?;
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
        if let Some(background) = &self.background {
            svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
            push_svg_number(&mut svg, self.logical_width, "SVG background width")?;
            svg.push_str("\" height=\"");
            push_svg_number(&mut svg, self.logical_height, "SVG background height")?;
            svg.push_str("\" fill=\"");
            push_escaped_attribute(&mut svg, background);
            svg.push_str("\"/>");
        }
        for element in &self.elements {
            match element {
                SvgElement::RoughRectangle {
                    source_id,
                    source_pointer,
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
                    if source_id.is_empty() || source_pointer.is_empty() {
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
                    push_escaped_attribute(&mut svg, stroke_color);
                    svg.push_str("\" stroke-width=\"");
                    push_svg_number(&mut svg, *stroke_width, "SVG stroke width")?;
                    svg.push_str("\" fill=\"none\"/></g>");
                }
            }
        }
        svg.push_str("</svg>");
        Ok(svg.into_bytes())
    }

    pub(super) fn primary_source(&self) -> Option<(&str, &str)> {
        self.elements.first().map(|element| match element {
            SvgElement::RoughRectangle {
                source_id,
                source_pointer,
                ..
            } => (source_id.as_str(), *source_pointer),
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

fn escaped_capacity(value: &str) -> Result<usize, FormatEncodeError> {
    value.len().checked_mul(6).ok_or_else(svg_size_error)
}

fn checked_capacity_add(current: usize, added: usize) -> Result<usize, FormatEncodeError> {
    current.checked_add(added).ok_or_else(svg_size_error)
}

fn svg_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("SVG plan exceeds serializer capacity".to_owned())
}
