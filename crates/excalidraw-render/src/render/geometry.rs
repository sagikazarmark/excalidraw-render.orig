//! Target-neutral geometry: the `Point`, `PathOperation` and `Path` primitives
//! every backend and the Rough.js port build on, the frame and image outline
//! shapes, element bounds, the arrowhead and elbow construction that feeds the
//! Rough generators, and the ECMAScript rounding the reference applies to
//! coordinates. `rough` owns only the Rough.js 4.6.4 generators themselves.

use crate::color::CanonicalColor;
use crate::normalization::{NormalizedElement, NormalizedElementKind};
use crate::ColorMode;

use super::resolve::ResolveSceneError;
use super::{freedraw, rough};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Point {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PathOperation {
    Move(Point),
    Line(Point),
    Cubic {
        control_1: Point,
        control_2: Point,
        end: Point,
    },
}

#[derive(Debug, PartialEq)]
pub(super) struct Path {
    pub(super) operations: Vec<PathOperation>,
}

impl Path {
    pub(super) fn cubic_bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut current = None;
        let mut bounds = None;
        for operation in &self.operations {
            match *operation {
                PathOperation::Move(point) | PathOperation::Line(point) => current = Some(point),
                PathOperation::Cubic {
                    control_1,
                    control_2,
                    end,
                } => {
                    let start = current?;
                    include_cubic_bounds(&mut bounds, start, control_1, control_2, end);
                    current = Some(end);
                }
            }
        }
        bounds
    }
}

fn include_cubic_bounds(
    bounds: &mut Option<(f64, f64, f64, f64)>,
    start: Point,
    control_1: Point,
    control_2: Point,
    end: Point,
) {
    let mut values_x = [start.x, end.x, 0.0, 0.0];
    let mut values_y = [start.y, end.y, 0.0, 0.0];
    let count_x = 2 + cubic_extrema(start.x, control_1.x, control_2.x, end.x, &mut values_x[2..]);
    let count_y = 2 + cubic_extrema(start.y, control_1.y, control_2.y, end.y, &mut values_y[2..]);
    let min_x = values_x[..count_x]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let min_y = values_y[..count_y]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let max_x = values_x[..count_x]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = values_y[..count_y]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    *bounds = Some(bounds.map_or((min_x, min_y, max_x, max_y), |current| {
        (
            current.0.min(min_x),
            current.1.min(min_y),
            current.2.max(max_x),
            current.3.max(max_y),
        )
    }));
}

fn cubic_extrema(
    start: f64,
    control_1: f64,
    control_2: f64,
    end: f64,
    output: &mut [f64],
) -> usize {
    let a = -start + 3.0 * control_1 - 3.0 * control_2 + end;
    let b = 2.0 * (start - 2.0 * control_1 + control_2);
    let c = control_1 - start;
    let mut count = 0;
    if a.abs() <= f64::EPSILON {
        if b != 0.0 {
            count += retain_cubic_extremum(
                -c / b,
                start,
                control_1,
                control_2,
                end,
                &mut output[count..],
            );
        }
        return count;
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return 0;
    }
    let root = discriminant.sqrt();
    count += retain_cubic_extremum(
        (-b + root) / (2.0 * a),
        start,
        control_1,
        control_2,
        end,
        &mut output[count..],
    );
    count += retain_cubic_extremum(
        (-b - root) / (2.0 * a),
        start,
        control_1,
        control_2,
        end,
        &mut output[count..],
    );
    count
}

fn retain_cubic_extremum(
    t: f64,
    start: f64,
    control_1: f64,
    control_2: f64,
    end: f64,
    output: &mut [f64],
) -> usize {
    if !(0.0..=1.0).contains(&t) || output.is_empty() {
        return 0;
    }
    let inverse = 1.0 - t;
    output[0] = inverse.powi(3) * start
        + 3.0 * inverse.powi(2) * t * control_1
        + 3.0 * inverse * t.powi(2) * control_2
        + t.powi(3) * end;
    1
}

pub(super) enum GeneratedGeometry {
    Rough(rough::Drawing),
    Freedraw(GeneratedFreedraw),
}

pub(super) struct GeneratedFreedraw {
    pub(super) path: freedraw::Path,
    pub(super) background_fill: Option<rough::Fill>,
}

impl GeneratedGeometry {
    pub(super) fn operation_count(&self) -> usize {
        match self {
            Self::Rough(drawing) => drawing.operation_count(),
            Self::Freedraw(freedraw) => {
                freedraw.path.operation_count()
                    + freedraw
                        .background_fill
                        .as_ref()
                        .map_or(0, |fill| fill.path.operations.len())
            }
        }
    }

    pub(super) fn bounds(&self, source: &NormalizedElement) -> (f64, f64, f64, f64) {
        match self {
            Self::Rough(drawing) => local_outline_bounds(source, Some(drawing)),
            Self::Freedraw(_) => source_layout_bounds(source),
        }
    }
}

pub(super) fn source_layout_bounds(source: &NormalizedElement) -> (f64, f64, f64, f64) {
    match &source.kind {
        NormalizedElementKind::Freedraw(freedraw) if !freedraw.points.is_empty() => {
            freedraw.points.iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(min_x, min_y, max_x, max_y), point| {
                    (
                        min_x.min(point[0]),
                        min_y.min(point[1]),
                        max_x.max(point[0]),
                        max_y.max(point[1]),
                    )
                },
            )
        }
        _ => (0.0, 0.0, source.width, source.height),
    }
}

/// `FRAME_STYLE.radius`: the corner radius of a frame's outline and of the clip
/// it applies to its children in a scene export.
pub(super) const FRAME_RADIUS: f64 = 8.0;

/// `FRAME_STYLE.strokeColor`: the stroke of a frame's outline in a scene export.
/// The reference's SVG paints every frame kind with it under its root dark-mode
/// filter; its canvas path recolors a magic frame's outline by theme
/// ([`magic_frame_canvas_outline_stroke_color`]) before the same filter.
pub(super) const FRAME_OUTLINE_STROKE_COLOR: CanonicalColor =
    CanonicalColor::from_rgb8(0xbb, 0xbb, 0xbb);

/// The stroke the reference's canvas path (`renderElement`) gives a magic
/// frame's outline: `#7affd7` in the light theme and `#1d8264` in the dark
/// theme, each then passing through the theme's filter like every other canvas
/// color. The PNG backend reproduces it; the reference's SVG does not recolor
/// magic frames.
pub(super) fn magic_frame_canvas_outline_stroke_color(mode: ColorMode) -> CanonicalColor {
    match mode {
        ColorMode::Light => CanonicalColor::from_rgb8(0x7a, 0xff, 0xd7),
        ColorMode::Dark => CanonicalColor::from_rgb8(0x1d, 0x82, 0x64),
    }
    .for_mode(mode)
}

/// `FRAME_STYLE.strokeWidth`: the width of a frame's outline stroke.
pub(super) const FRAME_OUTLINE_STROKE_WIDTH: f64 = 2.0;

pub(super) fn resolved_frame_outline(width: f64, height: f64) -> Result<Path, ResolveSceneError> {
    let radius = FRAME_RADIUS.min(width / 2.0).min(height / 2.0).max(0.0);
    rounded_rect_path(width, height, radius)
}

/// The exact rounded rectangle `(0, 0)`-`(width, height)` with `radius`
/// corners, as an SVG `<rect rx ry>` of that size draws it: four edges joined
/// by four kappa-approximated quarter arcs, starting and ending where the top
/// edge meets its left arc. Callers bound `radius` to half the shorter side.
pub(super) fn rounded_rect_path(
    width: f64,
    height: f64,
    radius: f64,
) -> Result<Path, ResolveSceneError> {
    let kappa = 0.552_284_749_830_793_6;
    let point = |x, y| Point { x, y };
    let mut operations = Vec::new();
    operations.try_reserve_exact(10)?;
    operations.push(PathOperation::Move(point(radius, 0.0)));
    operations.push(PathOperation::Line(point(width - radius, 0.0)));
    operations.push(PathOperation::Cubic {
        control_1: point(width - radius + radius * kappa, 0.0),
        control_2: point(width, radius - radius * kappa),
        end: point(width, radius),
    });
    operations.push(PathOperation::Line(point(width, height - radius)));
    operations.push(PathOperation::Cubic {
        control_1: point(width, height - radius + radius * kappa),
        control_2: point(width - radius + radius * kappa, height),
        end: point(width - radius, height),
    });
    operations.push(PathOperation::Line(point(radius, height)));
    operations.push(PathOperation::Cubic {
        control_1: point(radius - radius * kappa, height),
        control_2: point(0.0, height - radius + radius * kappa),
        end: point(0.0, height - radius),
    });
    operations.push(PathOperation::Line(point(0.0, radius)));
    operations.push(PathOperation::Cubic {
        control_1: point(0.0, radius - radius * kappa),
        control_2: point(radius - radius * kappa, 0.0),
        end: point(radius, 0.0),
    });
    Ok(Path { operations })
}

pub(super) fn resolved_frame_rect(width: f64, height: f64) -> Result<Path, ResolveSceneError> {
    let point = |x, y| Point { x, y };
    let mut operations = Vec::new();
    operations.try_reserve_exact(5)?;
    operations.push(PathOperation::Move(point(0.0, 0.0)));
    operations.push(PathOperation::Line(point(width, 0.0)));
    operations.push(PathOperation::Line(point(width, height)));
    operations.push(PathOperation::Line(point(0.0, height)));
    operations.push(PathOperation::Line(point(0.0, 0.0)));
    Ok(Path { operations })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn include_transformed_bounds(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
    min_x: &mut f64,
    min_y: &mut f64,
    max_x: &mut f64,
    max_y: &mut f64,
) {
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let sin = angle.sin();
    let cos = angle.cos();
    for (corner_x, corner_y) in [
        (x, y),
        (x + width, y),
        (x + width, y + height),
        (x, y + height),
    ] {
        let delta_x = corner_x - center_x;
        let delta_y = corner_y - center_y;
        let rotated_x = center_x + delta_x * cos - delta_y * sin;
        let rotated_y = center_y + delta_x * sin + delta_y * cos;
        *min_x = min_x.min(rotated_x);
        *min_y = min_y.min(rotated_y);
        *max_x = max_x.max(rotated_x);
        *max_y = max_y.max(rotated_y);
    }
}

/// The roughness Rough.js is given for `source`: Excalidraw's
/// `generateRoughOptions` tones small shapes down, halving the roughness for
/// a shape under 50 by 20 (a third under 10) and capping it at 2.5, and
/// exempts shapes rounded enough or, for linear elements, long enough for the
/// sketchiness to read.
pub(super) fn adjusted_roughness(source: &NormalizedElement) -> f64 {
    let max_size = source.width.max(source.height);
    let min_size = source.width.min(source.height);
    if min_size >= 20.0 && max_size >= 50.0
        || min_size >= 15.0 && source.roundness.is_some()
        || matches!(source.kind, NormalizedElementKind::Linear(_)) && max_size >= 50.0
    {
        source.roughness
    } else {
        (source.roughness / if max_size < 10.0 { 3.0 } else { 2.0 }).min(2.5)
    }
}

pub(super) fn rough_drawing(
    source: &NormalizedElement,
    roughness: f64,
    budget: &mut rough::GenerationBudget,
) -> Result<rough::Drawing, rough::GenerationError> {
    use rough::RoundedSegment;

    let width = source.width;
    let height = source.height;
    let preserve_vertices = source.roundness.is_some() || source.roughness < 2.0;
    let fill_style =
        (source.background_color != "transparent").then_some(match source.fill_style.as_str() {
            "hachure" => rough::FillStyle::Hachure,
            "cross-hatch" => rough::FillStyle::CrossHatch,
            "zigzag" => rough::FillStyle::Zigzag,
            _ => rough::FillStyle::Solid,
        });
    let disable_multi_stroke = source.stroke_style != "solid";
    match &source.kind {
        NormalizedElementKind::Rectangle | NormalizedElementKind::Embeddable { .. } => {
            let Some(roundness) = source.roundness else {
                return rough::rectangle_drawing(
                    width,
                    height,
                    source.seed,
                    roughness,
                    preserve_vertices,
                    disable_multi_stroke,
                    fill_style,
                    source.stroke_width,
                    budget,
                );
            };
            let radius = corner_radius(width.min(height), roundness);
            let start = Point { x: radius, y: 0.0 };
            let segments = [
                RoundedSegment::Line(Point {
                    x: width - radius,
                    y: 0.0,
                }),
                RoundedSegment::Quadratic {
                    control: Point { x: width, y: 0.0 },
                    end: Point {
                        x: width,
                        y: radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: width,
                    y: height - radius,
                }),
                RoundedSegment::Quadratic {
                    control: Point {
                        x: width,
                        y: height,
                    },
                    end: Point {
                        x: width - radius,
                        y: height,
                    },
                },
                RoundedSegment::Line(Point {
                    x: radius,
                    y: height,
                }),
                RoundedSegment::Quadratic {
                    control: Point { x: 0.0, y: height },
                    end: Point {
                        x: 0.0,
                        y: height - radius,
                    },
                },
                RoundedSegment::Line(Point { x: 0.0, y: radius }),
                RoundedSegment::Quadratic {
                    control: Point { x: 0.0, y: 0.0 },
                    end: Point { x: radius, y: 0.0 },
                },
            ];
            let fill_points = rough::rounded_fill_points(start, &segments, roughness, budget)?;
            rough::rounded_drawing(
                start,
                &segments,
                &fill_points,
                source.seed,
                roughness,
                preserve_vertices,
                disable_multi_stroke,
                fill_style,
                source.stroke_width,
                budget,
            )
        }
        NormalizedElementKind::Diamond => {
            // The official export places the diamond's vertices at
            // `floor(extent / 2) + 1`, not at the exact midpoint, so an
            // even-sized diamond sits one pixel right of and below centre.
            // Using the true midpoint moved every diamond outline.
            let mid_x = (width / 2.0).floor() + 1.0;
            let mid_y = (height / 2.0).floor() + 1.0;
            let points = [
                Point { x: mid_x, y: 0.0 },
                Point { x: width, y: mid_y },
                Point {
                    x: mid_x,
                    y: height,
                },
                Point { x: 0.0, y: mid_y },
            ];
            let Some(roundness) = source.roundness else {
                return rough::polygon_drawing(
                    &points,
                    source.seed,
                    roughness,
                    preserve_vertices,
                    disable_multi_stroke,
                    fill_style,
                    source.stroke_width,
                    budget,
                );
            };
            let vertical_radius = corner_radius(width / 2.0, roundness);
            let horizontal_radius = corner_radius(height / 2.0, roundness);
            let [top, right, bottom, left] = points;
            let start = Point {
                x: top.x + vertical_radius,
                y: top.y + horizontal_radius,
            };
            let segments = [
                RoundedSegment::Line(Point {
                    x: right.x - vertical_radius,
                    y: right.y - horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: right,
                    control_2: right,
                    end: Point {
                        x: right.x - vertical_radius,
                        y: right.y + horizontal_radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: bottom.x + vertical_radius,
                    y: bottom.y - horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: bottom,
                    control_2: bottom,
                    end: Point {
                        x: bottom.x - vertical_radius,
                        y: bottom.y - horizontal_radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: left.x + vertical_radius,
                    y: left.y + horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: left,
                    control_2: left,
                    end: Point {
                        x: left.x + vertical_radius,
                        y: left.y - horizontal_radius,
                    },
                },
                RoundedSegment::Line(Point {
                    x: top.x - vertical_radius,
                    y: top.y + horizontal_radius,
                }),
                RoundedSegment::Cubic {
                    control_1: top,
                    control_2: top,
                    end: Point {
                        x: top.x + vertical_radius,
                        y: top.y + horizontal_radius,
                    },
                },
            ];
            let fill_points = rough::rounded_fill_points(start, &segments, roughness, budget)?;
            rough::rounded_drawing(
                start,
                &segments,
                &fill_points,
                source.seed,
                roughness,
                preserve_vertices,
                disable_multi_stroke,
                fill_style,
                source.stroke_width,
                budget,
            )
        }
        NormalizedElementKind::Ellipse => rough::ellipse_drawing(
            width,
            height,
            source.seed,
            roughness,
            preserve_vertices,
            disable_multi_stroke,
            fill_style,
            source.stroke_width,
            budget,
        ),
        NormalizedElementKind::Linear(linear) => {
            let mut points = Vec::new();
            budget.work(u64::try_from(linear.points.len()).unwrap_or(u64::MAX))?;
            points.try_reserve_exact(linear.points.len())?;
            points.extend(linear.points.iter().map(|point| Point {
                x: point[0],
                y: point[1],
            }));
            const LINE_CONFIRM_THRESHOLD: f64 = 8.0;
            let closed = points.len() >= 3
                && points
                    .first()
                    .zip(points.last())
                    .is_some_and(|(first, last)| {
                        (first.x - last.x).hypot(first.y - last.y) <= LINE_CONFIRM_THRESHOLD
                    });
            let mut drawing = if linear.elbowed {
                elbow_drawing(
                    &points,
                    source.seed,
                    roughness,
                    disable_multi_stroke,
                    budget,
                )?
            } else if source.roundness.is_some() {
                rough::curve_drawing(
                    &points,
                    closed,
                    source.seed,
                    roughness,
                    disable_multi_stroke,
                    closed.then_some(fill_style).flatten(),
                    source.stroke_width,
                    budget,
                )?
            } else {
                if closed && fill_style.is_some() {
                    rough::polygon_drawing(
                        &points,
                        source.seed,
                        roughness,
                        preserve_vertices,
                        disable_multi_stroke,
                        fill_style,
                        source.stroke_width,
                        budget,
                    )?
                } else {
                    rough::linear_drawing(
                        &points,
                        closed,
                        source.seed,
                        roughness,
                        preserve_vertices,
                        disable_multi_stroke,
                        None,
                        source.stroke_width,
                        budget,
                    )?
                }
            };
            if matches!(
                linear.kind,
                crate::normalization::NormalizedLinearKind::Arrow
            ) {
                add_arrowheads(&mut drawing, linear, source, roughness, budget)?;
            }
            Ok(drawing)
        }
        NormalizedElementKind::Freedraw(_)
        | NormalizedElementKind::Text(_)
        | NormalizedElementKind::Image(_)
        | NormalizedElementKind::Frame(_)
        | NormalizedElementKind::MagicFrame(_) => {
            unreachable!("only outlines reach Rough geometry")
        }
    }
}

fn elbow_drawing(
    points: &[Point],
    seed: i32,
    roughness: f64,
    disable_multi_stroke: bool,
    budget: &mut rough::GenerationBudget,
) -> Result<rough::Drawing, rough::GenerationError> {
    use rough::RoundedSegment;

    let start = points[0];
    let mut segments = Vec::new();
    segments.try_reserve_exact(points.len().saturating_mul(2))?;
    for index in 1..points.len().saturating_sub(1) {
        let previous = points[index - 1];
        let corner = points[index];
        let next = points[index + 1];
        let radius = 16.0_f64
            .min((corner.x - previous.x).hypot(corner.y - previous.y) / 2.0)
            .min((next.x - corner.x).hypot(next.y - corner.y) / 2.0);
        let previous_is_horizontal = (previous.x - corner.x).abs() >= (previous.y - corner.y).abs();
        let next_is_horizontal = (next.x - corner.x).abs() >= (next.y - corner.y).abs();
        let approach = if previous_is_horizontal {
            Point {
                x: corner.x
                    + if previous.x < corner.x {
                        -radius
                    } else {
                        radius
                    },
                y: corner.y,
            }
        } else {
            Point {
                x: corner.x,
                y: corner.y
                    + if previous.y < corner.y {
                        -radius
                    } else {
                        radius
                    },
            }
        };
        let departure = if next_is_horizontal {
            Point {
                x: corner.x + if next.x < corner.x { -radius } else { radius },
                y: corner.y,
            }
        } else {
            Point {
                x: corner.x,
                y: corner.y + if next.y < corner.y { -radius } else { radius },
            }
        };
        segments.push(RoundedSegment::Line(approach));
        segments.push(RoundedSegment::Quadratic {
            control: corner,
            end: departure,
        });
    }
    segments.push(RoundedSegment::Line(
        *points.last().unwrap_or(&Point { x: 0.0, y: 0.0 }),
    ));
    rough::rounded_drawing(
        start,
        &segments,
        points,
        seed,
        roughness,
        true,
        disable_multi_stroke,
        None,
        1.0,
        budget,
    )
}

fn add_arrowheads(
    drawing: &mut rough::Drawing,
    linear: &crate::normalization::NormalizedLinear,
    source: &NormalizedElement,
    roughness: f64,
    budget: &mut rough::GenerationBudget,
) -> Result<(), rough::GenerationError> {
    let mut arrowhead_roughness = roughness;
    for (start, arrowhead) in [
        (true, linear.start_arrowhead.as_deref()),
        (false, linear.end_arrowhead.as_deref()),
    ] {
        let Some(arrowhead) = arrowhead else { continue };
        let Some(points) = arrowhead_points(drawing, linear, start, arrowhead, source.stroke_width)
        else {
            continue;
        };
        arrowhead_roughness = arrowhead_roughness.min(
            if matches!(arrowhead, "dot" | "circle" | "circle_outline") {
                0.5
            } else {
                1.0
            },
        );
        let head_roughness = arrowhead_roughness;
        let preserve_vertices = source.roughness < 2.0;
        match arrowhead {
            "dot" | "circle" | "circle_outline" => {
                let [center_x, center_y, diameter, ..] = points;
                let mut head = rough::circle_drawing(
                    diameter,
                    source.seed,
                    head_roughness,
                    preserve_vertices,
                    source.stroke_style != "solid",
                    Some(rough::FillStyle::Solid),
                    source.stroke_width,
                    budget,
                )?;
                translate_drawing(
                    &mut head,
                    center_x - diameter / 2.0,
                    center_y - diameter / 2.0,
                );
                drawing.decorations.push(rough::Decoration {
                    drawing: head,
                    fill_paint: if arrowhead == "circle_outline" {
                        rough::DecorationFillPaint::Background
                    } else {
                        rough::DecorationFillPaint::Stroke
                    },
                    dotted: false,
                });
            }
            "triangle" | "triangle_outline" => {
                let [tip_x, tip_y, left_x, left_y, right_x, right_y, ..] = points;
                let head_points = [
                    Point { x: tip_x, y: tip_y },
                    Point {
                        x: left_x,
                        y: left_y,
                    },
                    Point {
                        x: right_x,
                        y: right_y,
                    },
                    Point { x: tip_x, y: tip_y },
                ];
                let head = rough::polygon_drawing(
                    &head_points,
                    source.seed,
                    head_roughness,
                    preserve_vertices,
                    source.stroke_style != "solid",
                    Some(rough::FillStyle::Solid),
                    source.stroke_width,
                    budget,
                )?;
                drawing.decorations.push(rough::Decoration {
                    drawing: head,
                    fill_paint: if arrowhead == "triangle_outline" {
                        rough::DecorationFillPaint::Background
                    } else {
                        rough::DecorationFillPaint::Stroke
                    },
                    dotted: false,
                });
            }
            "diamond" | "diamond_outline" => {
                let head_points = [
                    Point {
                        x: points[0],
                        y: points[1],
                    },
                    Point {
                        x: points[2],
                        y: points[3],
                    },
                    Point {
                        x: points[4],
                        y: points[5],
                    },
                    Point {
                        x: points[6],
                        y: points[7],
                    },
                    Point {
                        x: points[0],
                        y: points[1],
                    },
                ];
                let head = rough::polygon_drawing(
                    &head_points,
                    source.seed,
                    head_roughness,
                    preserve_vertices,
                    source.stroke_style != "solid",
                    Some(rough::FillStyle::Solid),
                    source.stroke_width,
                    budget,
                )?;
                drawing.decorations.push(rough::Decoration {
                    drawing: head,
                    fill_paint: if arrowhead == "diamond_outline" {
                        rough::DecorationFillPaint::Background
                    } else {
                        rough::DecorationFillPaint::Stroke
                    },
                    dotted: false,
                });
            }
            _ => {
                let crossbar = (arrowhead == "crowfoot_one_or_many")
                    .then(|| {
                        arrowhead_points(
                            drawing,
                            linear,
                            start,
                            "crowfoot_one",
                            source.stroke_width,
                        )
                    })
                    .flatten();
                add_open_arrowhead(drawing, source, arrowhead, points, head_roughness, budget)?;
                if let Some(crossbar) = crossbar {
                    add_open_arrowhead(
                        drawing,
                        source,
                        "crowfoot_one",
                        crossbar,
                        head_roughness,
                        budget,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn add_open_arrowhead(
    drawing: &mut rough::Drawing,
    source: &NormalizedElement,
    arrowhead: &str,
    points: [f64; 8],
    roughness: f64,
    budget: &mut rough::GenerationBudget,
) -> Result<(), rough::GenerationError> {
    let tip = Point {
        x: points[0],
        y: points[1],
    };
    let first = Point {
        x: points[2],
        y: points[3],
    };
    let second = Point {
        x: points[4],
        y: points[5],
    };
    let mut push_line = |start, end| -> Result<(), rough::GenerationError> {
        let head = rough::linear_drawing(
            &[start, end],
            false,
            source.seed,
            roughness,
            source.roughness < 2.0,
            source.stroke_style != "solid",
            None,
            source.stroke_width,
            budget,
        )?;
        drawing.decorations.push(rough::Decoration {
            drawing: head,
            fill_paint: rough::DecorationFillPaint::Stroke,
            dotted: source.stroke_style == "dotted",
        });
        Ok(())
    };
    if arrowhead == "crowfoot_one" {
        push_line(first, second)?;
    } else {
        push_line(first, tip)?;
        push_line(second, tip)?;
    }
    Ok(())
}

fn arrowhead_points(
    drawing: &rough::Drawing,
    linear: &crate::normalization::NormalizedLinear,
    start: bool,
    arrowhead: &str,
    stroke_width: f64,
) -> Option<[f64; 8]> {
    let operations = &drawing.outline.operations;
    let index = if start {
        1
    } else {
        operations.len().checked_sub(1)?
    };
    let PathOperation::Cubic {
        control_1,
        control_2,
        end,
    } = operations.get(index)?
    else {
        return None;
    };
    let p0 = match operations.get(index.checked_sub(1)?)? {
        PathOperation::Move(point) => *point,
        PathOperation::Cubic { end, .. } => *end,
        PathOperation::Line(point) => *point,
    };
    let p3 = *end;
    let equation = |coordinate: fn(Point) -> f64| {
        let t = 0.3_f64;
        (1.0 - t).powi(3) * coordinate(p3)
            + 3.0 * t * (1.0 - t).powi(2) * coordinate(*control_2)
            + 3.0 * t.powi(2) * (1.0 - t) * coordinate(*control_1)
            + t.powi(3) * coordinate(p0)
    };
    let near = Point {
        x: equation(|p| p.x),
        y: equation(|p| p.y),
    };
    let tip = if start { p0 } else { p3 };
    let distance = (tip.x - near.x).hypot(tip.y - near.y);
    if distance == 0.0 {
        return None;
    }
    let normal = Point {
        x: (tip.x - near.x) / distance,
        y: (tip.y - near.y) / distance,
    };
    let size: f64 = match arrowhead {
        "arrow" => 25.0,
        "diamond" | "diamond_outline" => 12.0,
        "crowfoot_many" | "crowfoot_one" | "crowfoot_one_or_many" => 20.0,
        _ => 15.0,
    };
    let endpoint_index = if start { 0 } else { linear.points.len() - 1 };
    let adjacent_index = if start { 1 } else { linear.points.len() - 2 };
    let segment_length = (linear.points[endpoint_index][0] - linear.points[adjacent_index][0])
        .hypot(linear.points[endpoint_index][1] - linear.points[adjacent_index][1]);
    let minimum = size.min(
        segment_length
            * if arrowhead.starts_with("diamond") {
                0.25
            } else {
                0.5
            },
    );
    let base = Point {
        x: tip.x - normal.x * minimum,
        y: tip.y - normal.y * minimum,
    };
    if matches!(arrowhead, "dot" | "circle" | "circle_outline") {
        return Some([
            tip.x,
            tip.y,
            minimum + stroke_width - 2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ]);
    }
    let angle = if arrowhead == "bar" {
        90.0
    } else if arrowhead == "arrow" {
        20.0
    } else {
        25.0
    };
    let rotate = |point: Point, center: Point, degrees: f64| {
        let radians = degrees.to_radians();
        let dx = point.x - center.x;
        let dy = point.y - center.y;
        Point {
            x: center.x + dx * radians.cos() - dy * radians.sin(),
            y: center.y + dx * radians.sin() + dy * radians.cos(),
        }
    };
    if matches!(arrowhead, "crowfoot_many" | "crowfoot_one_or_many") {
        let first = rotate(tip, base, -angle);
        let second = rotate(tip, base, angle);
        return Some([
            base.x, base.y, first.x, first.y, second.x, second.y, 0.0, 0.0,
        ]);
    }
    let first = rotate(base, tip, -angle);
    let second = rotate(base, tip, angle);
    if matches!(arrowhead, "diamond" | "diamond_outline") {
        let adjacent = linear.points[adjacent_index];
        let direction = if start {
            (adjacent[1] - tip.y).atan2(adjacent[0] - tip.x)
        } else {
            (tip.y - adjacent[1]).atan2(tip.x - adjacent[0])
        };
        let opposite = Point {
            x: tip.x + if start { minimum * 2.0 } else { -minimum * 2.0 } * direction.cos(),
            y: tip.y + if start { minimum * 2.0 } else { -minimum * 2.0 } * direction.sin(),
        };
        return Some([
            tip.x, tip.y, first.x, first.y, opposite.x, opposite.y, second.x, second.y,
        ]);
    }
    Some([tip.x, tip.y, first.x, first.y, second.x, second.y, 0.0, 0.0])
}

fn translate_drawing(drawing: &mut rough::Drawing, x: f64, y: f64) {
    fn translate(path: &mut Path, x: f64, y: f64) {
        for operation in &mut path.operations {
            match operation {
                PathOperation::Move(point) | PathOperation::Line(point) => {
                    point.x += x;
                    point.y += y;
                }
                PathOperation::Cubic {
                    control_1,
                    control_2,
                    end,
                } => {
                    for point in [control_1, control_2, end] {
                        point.x += x;
                        point.y += y;
                    }
                }
            }
        }
    }
    translate(&mut drawing.outline, x, y);
    if let Some(fill) = &mut drawing.fill {
        translate(&mut fill.path, x, y);
    }
}

fn local_outline_bounds(
    source: &NormalizedElement,
    drawing: Option<&rough::Drawing>,
) -> (f64, f64, f64, f64) {
    let NormalizedElementKind::Linear(linear) = &source.kind else {
        return (0.0, 0.0, source.width, source.height);
    };
    // An elbow arrow's extent comes from its declared orthogonal route, not from
    // the jittered path drawn along it. Taking the drawn curve's bounds made the
    // scene a fraction of a pixel taller than the official export, which is
    // enough to change a rasterized dimension when the extent sits near an
    // integer boundary. Other linear elements do use the drawn bounds.
    if !linear.elbowed {
        if let Some(bounds) = drawing.and_then(|drawing| drawing.outline.cubic_bounds()) {
            return bounds;
        }
    }
    linear.points.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), point| {
            (
                min_x.min(point[0]),
                min_y.min(point[1]),
                max_x.max(point[0]),
                max_y.max(point[1]),
            )
        },
    )
}

pub(super) fn union_bounds(
    first: Option<(f64, f64, f64, f64)>,
    second: Option<(f64, f64, f64, f64)>,
) -> Option<(f64, f64, f64, f64)> {
    match (first, second) {
        (Some(first), Some(second)) => Some((
            first.0.min(second.0),
            first.1.min(second.1),
            first.2.max(second.2),
            first.3.max(second.3),
        )),
        (Some(bounds), None) | (None, Some(bounds)) => Some(bounds),
        (None, None) => None,
    }
}

/// The corner radius the reference clips an image element to:
/// `getCornerRadius(Math.min(element.width, element.height), element)` in both
/// its SVG `<clipPath>` and its canvas `roundRect` clip, and 0 without a
/// roundness record.
pub(super) fn image_corner_radius(
    width: f64,
    height: f64,
    roundness: Option<crate::normalization::NormalizedRoundness>,
) -> f64 {
    roundness.map_or(0.0, |roundness| corner_radius(width.min(height), roundness))
}

fn corner_radius(size: f64, roundness: crate::normalization::NormalizedRoundness) -> f64 {
    const PROPORTIONAL_RADIUS: f64 = 0.25;
    match roundness {
        crate::normalization::NormalizedRoundness::Proportional => size * PROPORTIONAL_RADIUS,
        crate::normalization::NormalizedRoundness::Adaptive { radius } => {
            radius.min(size * PROPORTIONAL_RADIUS)
        }
    }
}

/// ECMAScript `Math.round`: the closest integral value, a tie going toward
/// positive infinity, so `-2.5` is `-2` where `f64::round` gives `-3`. A
/// result of negative zero is canonicalized to zero. The fraction is compared
/// rather than `(value + 0.5).floor()` taken, which would carry
/// `0.49999999999999994` up to 1.
pub(super) fn ecmascript_round(value: f64) -> f64 {
    let floor = value.floor();
    let rounded = if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    };
    if rounded == 0.0 {
        0.0
    } else {
        rounded
    }
}

#[cfg(test)]
mod tests {
    use super::ecmascript_round;

    #[test]
    fn ecmascript_round_carries_ties_up_and_leaves_the_near_half_alone() {
        assert_eq!(ecmascript_round(2.5), 3.0);
        assert_eq!(ecmascript_round(-1.5), -1.0);
        assert_eq!(ecmascript_round(-2.5), -2.0);
        assert_eq!(ecmascript_round(-0.5).to_bits(), 0.0_f64.to_bits());
        assert_eq!(ecmascript_round(40.4), 40.0);
        // `(value + 0.5).floor()` would carry the largest double below one
        // half up to 1; `Math.round` does not.
        assert_eq!(ecmascript_round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(ecmascript_round(96.0), 96.0);
    }
}
