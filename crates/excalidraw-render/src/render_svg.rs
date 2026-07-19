use std::fmt::Write as _;

use anyhow::{ensure, Result};
use euclid::default::Point2D;
use roughr::core::{Drawable, FillStyle, OpSet, OpSetType, OpType, Options};
use roughr::generator::Generator;
use roughr::Srgba;

use crate::model::{is_transparent, Element, ExcalidrawFile};

const ARROWHEAD_LENGTH: f64 = 22.0;
const ARROWHEAD_ANGLE: f64 = std::f64::consts::PI / 6.0;

pub struct SvgDocument {
    pub content: String,
    pub width: f64,
    pub height: f64,
}

pub fn render(document: &ExcalidrawFile, padding: f64) -> Result<SvgDocument> {
    ensure!(
        padding.is_finite() && padding >= 0.0,
        "padding must be a finite non-negative number"
    );

    let elements: Vec<&Element> = document
        .elements
        .iter()
        .filter(|element| !element.is_deleted && element.is_supported())
        .filter(|element| geometry_points(element).is_some())
        .collect();

    let mut bounds = Bounds::default();
    for element in &elements {
        if let Some(points) = geometry_points(element) {
            let (cx, cy) = element.center();
            for point in points {
                let (x, y) = rotate_point(point.x, point.y, cx, cy, element.angle);
                bounds.include(x, y);
            }
        }
    }

    if bounds.is_empty() {
        bounds.include(0.0, 0.0);
        bounds.include(1.0, 1.0);
    }

    let raw_width = bounds.max_x - bounds.min_x + padding * 2.0;
    let raw_height = bounds.max_y - bounds.min_y + padding * 2.0;
    ensure!(
        raw_width.is_finite() && raw_height.is_finite(),
        "rendered bounds are too large"
    );
    let width = raw_width.max(1.0);
    let height = raw_height.max(1.0);
    let min_x = bounds.min_x - padding - (width - raw_width) / 2.0;
    let min_y = bounds.min_y - padding - (height - raw_height) / 2.0;

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="{} {} {} {}">"#,
        number(width),
        number(height),
        number(min_x),
        number(min_y),
        number(width),
        number(height)
    );

    if let Some(background) = document
        .app_state
        .view_background_color
        .as_deref()
        .filter(|color| !color.is_empty() && !is_transparent(color))
    {
        let _ = writeln!(
            svg,
            r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
            number(min_x),
            number(min_y),
            number(width),
            number(height),
            escape_attribute(background)
        );
    }

    let generator = Generator::default();
    for element in elements {
        if let Some(drawable) = generate_drawable(&generator, element) {
            append_element(&mut svg, element, &drawable);
        }
    }

    svg.push_str("</svg>\n");
    Ok(SvgDocument {
        content: svg,
        width,
        height,
    })
}

fn generate_drawable(generator: &Generator, element: &Element) -> Option<Drawable<f64>> {
    let options = Some(rough_options(element));
    match element.element_type.as_str() {
        "rectangle" => Some(generator.rectangle(
            element.x,
            element.y,
            element.width,
            element.height,
            &options,
        )),
        "ellipse" => {
            let (cx, cy) = element.center();
            Some(generator.ellipse(cx, cy, element.width, element.height, &options))
        }
        "diamond" => {
            let (cx, cy) = element.center();
            let points = [
                Point2D::new(cx, element.y),
                Point2D::new(element.x + element.width, cy),
                Point2D::new(cx, element.y + element.height),
                Point2D::new(element.x, cy),
            ];
            Some(generator.polygon(&points, &options))
        }
        "line" | "arrow" => {
            let points = linear_points(element)?;
            let mut drawable = generator.linear_path(&points, false, &options);
            if element.element_type == "arrow" {
                if let Some(head) = arrowhead_points(&points) {
                    let arrowhead = generator.linear_path(&head, false, &options);
                    drawable.sets.extend(arrowhead.sets);
                }
            }
            Some(drawable)
        }
        _ => None,
    }
}

fn rough_options(element: &Element) -> Options {
    let fill_style = match element.fill_style.as_deref() {
        Some("solid") => FillStyle::Solid,
        Some("cross-hatch") => FillStyle::CrossHatch,
        Some("hachure" | "zigzag") | None => FillStyle::Hachure,
        Some(_) => FillStyle::Hachure,
    };
    let seed = element
        .seed
        .map(|seed| seed as u64)
        .unwrap_or_else(|| stable_id_seed(&element.id));

    Options {
        stroke: Some(rough_color(element.stroke_color())),
        stroke_width: Some(element.stroke_width() as f32),
        fill: element.fill_color().map(rough_color),
        fill_style: Some(fill_style),
        roughness: Some(element.roughness() as f32),
        seed: Some(seed),
        ..Options::default()
    }
}

fn append_element(svg: &mut String, element: &Element, drawable: &Drawable<f64>) {
    let (cx, cy) = element.center();
    let _ = writeln!(
        svg,
        r#"  <g data-element-id="{}" transform="rotate({} {} {})" opacity="{}">"#,
        escape_attribute(&element.id),
        number(element.angle.to_degrees()),
        number(cx),
        number(cy),
        number(element.opacity())
    );

    for set in &drawable.sets {
        let path = ops_to_path(set);
        if path.is_empty() {
            continue;
        }

        match set.op_set_type {
            OpSetType::Path => {
                let dash = stroke_dasharray(element);
                let _ = writeln!(
                    svg,
                    r#"    <path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"{}/>"#,
                    path,
                    escape_attribute(element.stroke_color()),
                    number(element.stroke_width()),
                    dash
                );
            }
            OpSetType::FillPath => {
                if let Some(fill) = element.fill_color() {
                    let _ = writeln!(
                        svg,
                        r#"    <path d="{}" fill="{}" stroke="none"/>"#,
                        path,
                        escape_attribute(fill)
                    );
                }
            }
            OpSetType::FillSketch => {
                if let Some(fill) = element.fill_color() {
                    let fill_width = drawable
                        .options
                        .fill_weight
                        .filter(|weight| *weight >= 0.0)
                        .map(f64::from)
                        .unwrap_or_else(|| element.stroke_width() / 2.0);
                    let _ = writeln!(
                        svg,
                        r#"    <path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                        path,
                        escape_attribute(fill),
                        number(fill_width)
                    );
                }
            }
        }
    }

    svg.push_str("  </g>\n");
}

fn ops_to_path(set: &OpSet<f64>) -> String {
    let mut path = String::new();
    for operation in &set.ops {
        match operation.op {
            OpType::Move => {
                if let [x, y, ..] = operation.data.as_slice() {
                    let _ = write!(path, "M{} {}", number(*x), number(*y));
                }
            }
            OpType::LineTo => {
                if let [x, y, ..] = operation.data.as_slice() {
                    let _ = write!(path, "L{} {}", number(*x), number(*y));
                }
            }
            OpType::BCurveTo => {
                if let [x1, y1, x2, y2, x, y, ..] = operation.data.as_slice() {
                    let _ = write!(
                        path,
                        "C{} {} {} {} {} {}",
                        number(*x1),
                        number(*y1),
                        number(*x2),
                        number(*y2),
                        number(*x),
                        number(*y)
                    );
                }
            }
        }
    }
    path
}

fn geometry_points(element: &Element) -> Option<Vec<Point2D<f64>>> {
    if ![
        element.x,
        element.y,
        element.width,
        element.height,
        element.angle,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        return None;
    }

    match element.element_type.as_str() {
        "rectangle" | "ellipse" => Some(vec![
            Point2D::new(element.x, element.y),
            Point2D::new(element.x + element.width, element.y),
            Point2D::new(element.x + element.width, element.y + element.height),
            Point2D::new(element.x, element.y + element.height),
        ]),
        "diamond" => {
            let (cx, cy) = element.center();
            Some(vec![
                Point2D::new(cx, element.y),
                Point2D::new(element.x + element.width, cy),
                Point2D::new(cx, element.y + element.height),
                Point2D::new(element.x, cy),
            ])
        }
        "line" | "arrow" => {
            let points = linear_points(element)?;
            let mut geometry = points.clone();
            if element.element_type == "arrow" {
                if let Some(head) = arrowhead_points(&points) {
                    geometry.extend(head);
                }
            }
            Some(geometry)
        }
        _ => None,
    }
}

fn linear_points(element: &Element) -> Option<Vec<Point2D<f64>>> {
    if element.points.len() < 2 {
        return None;
    }

    element
        .points
        .iter()
        .map(|point| {
            let x = element.x + point[0];
            let y = element.y + point[1];
            (x.is_finite() && y.is_finite()).then_some(Point2D::new(x, y))
        })
        .collect()
}

fn arrowhead_points(points: &[Point2D<f64>]) -> Option<Vec<Point2D<f64>>> {
    let end = *points.last()?;
    let previous = points
        .iter()
        .rev()
        .skip(1)
        .copied()
        .find(|point| point.x != end.x || point.y != end.y)?;
    let incoming = (end.y - previous.y).atan2(end.x - previous.x);
    let backward = incoming + std::f64::consts::PI;
    let left = Point2D::new(
        end.x + ARROWHEAD_LENGTH * (backward - ARROWHEAD_ANGLE).cos(),
        end.y + ARROWHEAD_LENGTH * (backward - ARROWHEAD_ANGLE).sin(),
    );
    let right = Point2D::new(
        end.x + ARROWHEAD_LENGTH * (backward + ARROWHEAD_ANGLE).cos(),
        end.y + ARROWHEAD_LENGTH * (backward + ARROWHEAD_ANGLE).sin(),
    );
    Some(vec![left, end, right])
}

fn rotate_point(x: f64, y: f64, cx: f64, cy: f64, angle: f64) -> (f64, f64) {
    let sin = angle.sin();
    let cos = angle.cos();
    let dx = x - cx;
    let dy = y - cy;
    (cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
}

fn stroke_dasharray(element: &Element) -> String {
    match element.stroke_style.as_deref() {
        Some("dashed") => r#" stroke-dasharray="8 8""#.to_owned(),
        Some("dotted") => r#" stroke-dasharray="2 6""#.to_owned(),
        _ => String::new(),
    }
}

fn rough_color(color: &str) -> Srgba {
    let rgba = parse_hex_color(color).unwrap_or([0, 0, 0, 255]);
    Srgba::new(
        f32::from(rgba[0]) / 255.0,
        f32::from(rgba[1]) / 255.0,
        f32::from(rgba[2]) / 255.0,
        f32::from(rgba[3]) / 255.0,
    )
}

fn parse_hex_color(color: &str) -> Option<[u8; 4]> {
    let hex = color.trim().strip_prefix('#')?;
    if !hex.is_ascii() {
        return None;
    }
    match hex.len() {
        3 => Some([
            duplicate_hex(hex, 0)?,
            duplicate_hex(hex, 1)?,
            duplicate_hex(hex, 2)?,
            255,
        ]),
        4 => Some([
            duplicate_hex(hex, 0)?,
            duplicate_hex(hex, 1)?,
            duplicate_hex(hex, 2)?,
            duplicate_hex(hex, 3)?,
        ]),
        6 | 8 => {
            let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let alpha = if hex.len() == 8 {
                u8::from_str_radix(&hex[6..8], 16).ok()?
            } else {
                255
            };
            Some([red, green, blue, alpha])
        }
        _ => None,
    }
}

fn duplicate_hex(hex: &str, index: usize) -> Option<u8> {
    let digit = hex.as_bytes().get(index).copied()?;
    let value = (digit as char).to_digit(16)? as u8;
    Some(value * 17)
}

fn stable_id_seed(id: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn number(value: f64) -> String {
    let value = if value.abs() < 0.0005 { 0.0 } else { value };
    let mut text = format!("{value:.3}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

#[derive(Default)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    initialized: bool,
}

impl Bounds {
    fn include(&mut self, x: f64, y: f64) {
        if !self.initialized {
            self.min_x = x;
            self.min_y = y;
            self.max_x = x;
            self.max_y = y;
            self.initialized = true;
            return;
        }

        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }

    fn is_empty(&self) -> bool {
        !self.initialized
    }
}
