use crate::{RenderRequest, Sizing};

use super::{format_finite_number, ResolvedScene};

pub(super) struct SvgPlan {
    logical_width: f64,
    logical_height: f64,
    output_width: f64,
    output_height: f64,
    include_background: bool,
}

impl SvgPlan {
    pub(super) fn new(scene: &ResolvedScene, request: &RenderRequest) -> Self {
        let scale = match request.sizing() {
            Sizing::Scale(scale) => scale,
            Sizing::MaxDimension(_) => 1.0,
        };
        Self {
            logical_width: scene.logical_width,
            logical_height: scene.logical_height,
            output_width: scene.logical_width * scale,
            output_height: scene.logical_height * scale,
            include_background: scene.include_background,
        }
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, String> {
        let logical_width = format_finite_number(self.logical_width, "SVG width")?;
        let logical_height = format_finite_number(self.logical_height, "SVG height")?;
        let output_width = format_finite_number(self.output_width, "scaled SVG width")?;
        let output_height = format_finite_number(self.output_height, "scaled SVG height")?;
        let mut svg = String::new();
        svg.try_reserve(256)
            .map_err(|_| "failed to allocate SVG output".to_owned())?;
        svg.push_str("<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 ");
        svg.push_str(&logical_width);
        svg.push(' ');
        svg.push_str(&logical_height);
        svg.push_str("\" width=\"");
        svg.push_str(&output_width);
        svg.push_str("\" height=\"");
        svg.push_str(&output_height);
        svg.push_str("\">");
        if self.include_background {
            svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
            svg.push_str(&logical_width);
            svg.push_str("\" height=\"");
            svg.push_str(&logical_height);
            svg.push_str("\" fill=\"#ffffff\"/>");
        }
        svg.push_str("</svg>");
        Ok(svg.into_bytes())
    }
}
