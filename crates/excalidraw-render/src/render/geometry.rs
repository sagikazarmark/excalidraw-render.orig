use super::*;

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

pub(super) fn resolved_frame_outline(
    width: f64,
    height: f64,
) -> Result<rough::Path, ResolveSceneError> {
    let radius = 8.0_f64.min(width / 2.0).min(height / 2.0).max(0.0);
    let kappa = 0.552_284_749_830_793_6;
    let point = |x, y| rough::Point { x, y };
    let mut operations = Vec::new();
    operations.try_reserve_exact(10)?;
    operations.push(rough::PathOperation::Move(point(radius, 0.0)));
    operations.push(rough::PathOperation::Line(point(width - radius, 0.0)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(width - radius + radius * kappa, 0.0),
        control_2: point(width, radius - radius * kappa),
        end: point(width, radius),
    });
    operations.push(rough::PathOperation::Line(point(width, height - radius)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(width, height - radius + radius * kappa),
        control_2: point(width - radius + radius * kappa, height),
        end: point(width - radius, height),
    });
    operations.push(rough::PathOperation::Line(point(radius, height)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(radius - radius * kappa, height),
        control_2: point(0.0, height - radius + radius * kappa),
        end: point(0.0, height - radius),
    });
    operations.push(rough::PathOperation::Line(point(0.0, radius)));
    operations.push(rough::PathOperation::Cubic {
        control_1: point(0.0, radius - radius * kappa),
        control_2: point(radius - radius * kappa, 0.0),
        end: point(radius, 0.0),
    });
    Ok(rough::Path { operations })
}

pub(super) fn resolved_frame_rect(
    width: f64,
    height: f64,
) -> Result<rough::Path, ResolveSceneError> {
    let point = |x, y| rough::Point { x, y };
    let mut operations = Vec::new();
    operations.try_reserve_exact(5)?;
    operations.push(rough::PathOperation::Move(point(0.0, 0.0)));
    operations.push(rough::PathOperation::Line(point(width, 0.0)));
    operations.push(rough::PathOperation::Line(point(width, height)));
    operations.push(rough::PathOperation::Line(point(0.0, height)));
    operations.push(rough::PathOperation::Line(point(0.0, 0.0)));
    Ok(rough::Path { operations })
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

pub(super) fn rough_drawing(
    source: &NormalizedElement,
    roughness: f64,
    budget: &mut rough::GenerationBudget,
) -> Result<rough::Drawing, rough::GenerationError> {
    use rough::{Point, RoundedSegment};

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
                Point {
                    x: width,
                    y: mid_y,
                },
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
    points: &[rough::Point],
    seed: i32,
    roughness: f64,
    disable_multi_stroke: bool,
    budget: &mut rough::GenerationBudget,
) -> Result<rough::Drawing, rough::GenerationError> {
    use rough::{Point, RoundedSegment};

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
            rough::Point {
                x: corner.x
                    + if previous.x < corner.x {
                        -radius
                    } else {
                        radius
                    },
                y: corner.y,
            }
        } else {
            rough::Point {
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
            rough::Point {
                x: corner.x + if next.x < corner.x { -radius } else { radius },
                y: corner.y,
            }
        } else {
            rough::Point {
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
                    rough::Point { x: tip_x, y: tip_y },
                    rough::Point {
                        x: left_x,
                        y: left_y,
                    },
                    rough::Point {
                        x: right_x,
                        y: right_y,
                    },
                    rough::Point { x: tip_x, y: tip_y },
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
                    rough::Point {
                        x: points[0],
                        y: points[1],
                    },
                    rough::Point {
                        x: points[2],
                        y: points[3],
                    },
                    rough::Point {
                        x: points[4],
                        y: points[5],
                    },
                    rough::Point {
                        x: points[6],
                        y: points[7],
                    },
                    rough::Point {
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
    let tip = rough::Point {
        x: points[0],
        y: points[1],
    };
    let first = rough::Point {
        x: points[2],
        y: points[3],
    };
    let second = rough::Point {
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
    use rough::PathOperation;
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
    let equation = |coordinate: fn(rough::Point) -> f64| {
        let t = 0.3_f64;
        (1.0 - t).powi(3) * coordinate(p3)
            + 3.0 * t * (1.0 - t).powi(2) * coordinate(*control_2)
            + 3.0 * t.powi(2) * (1.0 - t) * coordinate(*control_1)
            + t.powi(3) * coordinate(p0)
    };
    let near = rough::Point {
        x: equation(|p| p.x),
        y: equation(|p| p.y),
    };
    let tip = if start { p0 } else { p3 };
    let distance = (tip.x - near.x).hypot(tip.y - near.y);
    if distance == 0.0 {
        return None;
    }
    let normal = rough::Point {
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
    let base = rough::Point {
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
    let rotate = |point: rough::Point, center: rough::Point, degrees: f64| {
        let radians = degrees.to_radians();
        let dx = point.x - center.x;
        let dy = point.y - center.y;
        rough::Point {
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
        let opposite = rough::Point {
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
    fn translate(path: &mut rough::Path, x: f64, y: f64) {
        for operation in &mut path.operations {
            match operation {
                rough::PathOperation::Move(point) | rough::PathOperation::Line(point) => {
                    point.x += x;
                    point.y += y;
                }
                rough::PathOperation::Cubic {
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

fn corner_radius(size: f64, roundness: crate::normalization::NormalizedRoundness) -> f64 {
    const PROPORTIONAL_RADIUS: f64 = 0.25;
    match roundness {
        crate::normalization::NormalizedRoundness::Proportional => size * PROPORTIONAL_RADIUS,
        crate::normalization::NormalizedRoundness::Adaptive { radius } => {
            radius.min(size * PROPORTIONAL_RADIUS)
        }
    }
}
