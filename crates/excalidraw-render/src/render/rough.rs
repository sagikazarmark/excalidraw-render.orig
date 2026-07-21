use std::collections::TryReserveError;
use std::fmt::Write as _;

use super::FormatEncodeError;

#[derive(Clone, Copy)]
pub(super) struct Point {
    pub(super) x: f64,
    pub(super) y: f64,
}

pub(super) enum PathOperation {
    Move(Point),
    Cubic {
        control_1: Point,
        control_2: Point,
        end: Point,
    },
}

pub(super) struct Path {
    pub(super) operations: Vec<PathOperation>,
}

pub(super) fn push_number(output: &mut String, value: f64) -> Result<(), FormatEncodeError> {
    let (negative, units) = fixed_two_decimal_units(value)?;
    if negative && units != 0 {
        output.push('-');
    }
    let whole = units / 100;
    let fraction = units % 100;
    write!(output, "{whole}")
        .map_err(|_| FormatEncodeError::Encoding("failed to write Rough path number".to_owned()))?;
    if fraction != 0 {
        output.push('.');
        if fraction % 10 == 0 {
            write!(output, "{}", fraction / 10)
        } else {
            write!(output, "{fraction:02}")
        }
        .map_err(|_| {
            FormatEncodeError::Encoding("failed to write Rough path fraction".to_owned())
        })?;
    }
    Ok(())
}

fn fixed_two_decimal_units(value: f64) -> Result<(bool, u128), FormatEncodeError> {
    if !value.is_finite() || value.abs() >= 1e21 {
        return Err(FormatEncodeError::Encoding(
            "Rough path number is outside fixed-decimal range".to_owned(),
        ));
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent_bits == 0 && fraction == 0 {
        return Ok((false, 0));
    }
    let (significand, exponent) = if exponent_bits == 0 {
        (u128::from(fraction), -1074)
    } else {
        (
            u128::from(fraction | (1_u64 << 52)),
            exponent_bits - 1023 - 52,
        )
    };
    let scaled = significand.checked_mul(100).ok_or_else(number_error)?;
    let units = if exponent >= 0 {
        scaled
            .checked_shl(u32::try_from(exponent).map_err(|_| number_error())?)
            .ok_or_else(number_error)?
    } else {
        let denominator_shift = exponent.unsigned_abs();
        if denominator_shift >= 128 {
            0
        } else {
            let denominator = 1_u128 << denominator_shift;
            let quotient = scaled / denominator;
            let remainder = scaled % denominator;
            quotient + u128::from(remainder >= denominator - remainder)
        }
    };
    Ok((negative, units))
}

fn number_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("Rough path number exceeds serializer capacity".to_owned())
}

pub(super) fn rectangle(
    width: f64,
    height: f64,
    seed: i32,
    roughness: f64,
    preserve_vertices: bool,
) -> Result<Path, TryReserveError> {
    let top_left = Point { x: 0.0, y: 0.0 };
    let top_right = Point { x: width, y: 0.0 };
    let bottom_right = Point {
        x: width,
        y: height,
    };
    let bottom_left = Point { x: 0.0, y: height };
    let segments = [
        (top_left, top_right),
        (top_right, bottom_right),
        (bottom_right, bottom_left),
        (bottom_left, top_left),
    ];
    let mut random = Random::new(seed);
    let mut operations = Vec::new();
    operations.try_reserve_exact(16)?;
    for (start, end) in segments {
        double_line(
            start,
            end,
            roughness,
            preserve_vertices,
            &mut random,
            &mut operations,
        );
    }
    Ok(Path { operations })
}

pub(super) fn polygon(
    points: &[Point],
    seed: i32,
    roughness: f64,
    preserve_vertices: bool,
) -> Result<Path, TryReserveError> {
    linear_path(points, true, seed, roughness, preserve_vertices)
}

pub(super) fn linear_path(
    points: &[Point],
    close: bool,
    seed: i32,
    roughness: f64,
    preserve_vertices: bool,
) -> Result<Path, TryReserveError> {
    let segment_count = points.len().saturating_sub(1) + usize::from(close && points.len() > 2);
    let mut operations = Vec::new();
    operations.try_reserve_exact(segment_count.saturating_mul(4))?;
    let mut random = Random::new(seed);
    for points in points.windows(2) {
        double_line(
            points[0],
            points[1],
            roughness,
            preserve_vertices,
            &mut random,
            &mut operations,
        );
    }
    if close && points.len() > 2 {
        double_line(
            *points.last().expect("a closed path has points"),
            points[0],
            roughness,
            preserve_vertices,
            &mut random,
            &mut operations,
        );
    }
    Ok(Path { operations })
}

pub(super) fn rounded_path(
    start: Point,
    segments: &[RoundedSegment],
    seed: i32,
    roughness: f64,
    preserve_vertices: bool,
) -> Result<Path, TryReserveError> {
    let mut operations = Vec::new();
    operations.try_reserve_exact(segments.len().saturating_mul(4))?;
    let mut random = Random::new(seed);
    let mut current = start;
    for segment in segments {
        match *segment {
            RoundedSegment::Line(end) => {
                double_line(
                    current,
                    end,
                    roughness,
                    preserve_vertices,
                    &mut random,
                    &mut operations,
                );
                current = end;
            }
            RoundedSegment::Quadratic { control, end } => {
                let control_1 = Point {
                    x: current.x + (control.x - current.x) * (2.0 / 3.0),
                    y: current.y + (control.y - current.y) * (2.0 / 3.0),
                };
                let control_2 = Point {
                    x: end.x + (control.x - end.x) * (2.0 / 3.0),
                    y: end.y + (control.y - end.y) * (2.0 / 3.0),
                };
                double_bezier(
                    current,
                    control_1,
                    control_2,
                    end,
                    roughness,
                    preserve_vertices,
                    &mut random,
                    &mut operations,
                );
                current = end;
            }
            RoundedSegment::Cubic {
                control_1,
                control_2,
                end,
            } => {
                double_bezier(
                    current,
                    control_1,
                    control_2,
                    end,
                    roughness,
                    preserve_vertices,
                    &mut random,
                    &mut operations,
                );
                current = end;
            }
        }
    }
    Ok(Path { operations })
}

#[derive(Clone, Copy)]
pub(super) enum RoundedSegment {
    Line(Point),
    Quadratic {
        control: Point,
        end: Point,
    },
    Cubic {
        control_1: Point,
        control_2: Point,
        end: Point,
    },
}

pub(super) fn curve(points: &[Point], seed: i32, roughness: f64) -> Result<Path, TryReserveError> {
    let mut operations = Vec::new();
    operations.try_reserve_exact(points.len().saturating_mul(4))?;
    curve_stroke(
        points,
        1.0 * (1.0 + roughness * 0.2),
        seed,
        roughness,
        &mut operations,
    )?;
    curve_stroke(
        points,
        1.5 * (1.0 + roughness * 0.22),
        seed.wrapping_add(1),
        roughness,
        &mut operations,
    )?;
    Ok(Path { operations })
}

fn curve_stroke(
    points: &[Point],
    offset: f64,
    seed: i32,
    roughness: f64,
    operations: &mut Vec<PathOperation>,
) -> Result<(), TryReserveError> {
    if points.is_empty() {
        return Ok(());
    }
    let mut random = Random::new(seed);
    let mut perturbed = Vec::new();
    perturbed.try_reserve_exact(points.len().saturating_add(2))?;
    let mut perturb = |point: Point| Point {
        x: point.x + random_offset(offset, roughness, 1.0, &mut random),
        y: point.y + random_offset(offset, roughness, 1.0, &mut random),
    };
    perturbed.push(perturb(points[0]));
    perturbed.push(perturb(points[0]));
    for (index, point) in points.iter().copied().enumerate().skip(1) {
        perturbed.push(perturb(point));
        if index == points.len() - 1 {
            perturbed.push(perturb(point));
        }
    }
    smooth_curve(&perturbed, operations);
    Ok(())
}

fn smooth_curve(points: &[Point], operations: &mut Vec<PathOperation>) {
    if points.len() == 3 {
        operations.push(PathOperation::Move(points[1]));
        operations.push(PathOperation::Cubic {
            control_1: points[1],
            control_2: points[2],
            end: points[2],
        });
        return;
    }
    if points.len() <= 3 {
        return;
    }
    operations.push(PathOperation::Move(points[1]));
    for index in 1..points.len() - 2 {
        let current = points[index];
        let next = points[index + 1];
        operations.push(PathOperation::Cubic {
            control_1: Point {
                x: current.x + (next.x - points[index - 1].x) / 6.0,
                y: current.y + (next.y - points[index - 1].y) / 6.0,
            },
            control_2: Point {
                x: next.x + (current.x - points[index + 2].x) / 6.0,
                y: next.y + (current.y - points[index + 2].y) / 6.0,
            },
            end: next,
        });
    }
}

pub(super) fn ellipse(
    width: f64,
    height: f64,
    seed: i32,
    roughness: f64,
) -> Result<Path, TryReserveError> {
    let perimeter = (std::f64::consts::PI
        * 2.0
        * (((width / 2.0).powi(2) + (height / 2.0).powi(2)) / 2.0).sqrt())
    .sqrt();
    let step_count = 9.0_f64.max((9.0 / 200.0_f64.sqrt()) * perimeter).ceil();
    let increment = std::f64::consts::TAU / step_count;
    let center = Point {
        x: width / 2.0,
        y: height / 2.0,
    };
    let mut random = Random::new(seed);
    // curveFitting is one for Excalidraw ellipses. Rough.js still consumes both zero offsets.
    let radius_x = width.abs() / 2.0 + random_offset(0.0, roughness, 1.0, &mut random);
    let radius_y = height.abs() / 2.0 + random_offset(0.0, roughness, 1.0, &mut random);
    let overlap_extent = random_range(0.4, 1.0, roughness, &mut random);
    let overlap = increment * random_range(0.1, overlap_extent, roughness, &mut random);
    let mut operations = Vec::new();
    operations.try_reserve_exact((step_count as usize).saturating_mul(4))?;
    if roughness == 0.0 {
        core_ellipse_stroke(center, radius_x, radius_y, increment / 4.0, &mut operations)?;
        return Ok(Path { operations });
    }
    ellipse_stroke(
        center,
        radius_x,
        radius_y,
        increment,
        1.0,
        overlap,
        roughness,
        &mut random,
        &mut operations,
    )?;
    ellipse_stroke(
        center,
        radius_x,
        radius_y,
        increment,
        1.5,
        0.0,
        roughness,
        &mut random,
        &mut operations,
    )?;
    Ok(Path { operations })
}

fn core_ellipse_stroke(
    center: Point,
    radius_x: f64,
    radius_y: f64,
    increment: f64,
    operations: &mut Vec<PathOperation>,
) -> Result<(), TryReserveError> {
    let point = |angle: f64| Point {
        x: center.x + radius_x * angle.cos(),
        y: center.y + radius_y * angle.sin(),
    };
    let mut points = Vec::new();
    points.try_reserve_exact((std::f64::consts::TAU / increment).ceil() as usize + 4)?;
    points.push(point(-increment));
    let mut angle = 0.0;
    while angle <= std::f64::consts::TAU {
        points.push(point(angle));
        angle += increment;
    }
    points.push(point(0.0));
    points.push(point(increment));
    smooth_curve(&points, operations);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ellipse_stroke(
    center: Point,
    radius_x: f64,
    radius_y: f64,
    increment: f64,
    offset: f64,
    overlap: f64,
    roughness: f64,
    random: &mut Random,
    operations: &mut Vec<PathOperation>,
) -> Result<(), TryReserveError> {
    let angle_offset = random_offset(0.5, roughness, 1.0, random) - std::f64::consts::FRAC_PI_2;
    let mut points = Vec::new();
    points.try_reserve_exact((std::f64::consts::TAU / increment).ceil() as usize + 5)?;
    let point = |angle: f64, scale: f64, random: &mut Random| Point {
        x: random_offset(offset, roughness, 1.0, random)
            + center.x
            + scale * radius_x * angle.cos(),
        y: random_offset(offset, roughness, 1.0, random)
            + center.y
            + scale * radius_y * angle.sin(),
    };
    points.push(point(angle_offset - increment, 0.9, random));
    let end_angle = std::f64::consts::TAU + angle_offset - 0.01;
    let mut angle = angle_offset;
    while angle < end_angle {
        points.push(point(angle, 1.0, random));
        angle += increment;
    }
    points.push(point(
        angle_offset + std::f64::consts::TAU + overlap * 0.5,
        1.0,
        random,
    ));
    points.push(point(angle_offset + overlap, 0.98, random));
    points.push(point(angle_offset + overlap * 0.5, 0.9, random));
    smooth_curve(&points, operations);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn double_bezier(
    start: Point,
    control_1: Point,
    control_2: Point,
    end: Point,
    roughness: f64,
    preserve_vertices: bool,
    random: &mut Random,
    operations: &mut Vec<PathOperation>,
) {
    for (index, extent) in [2.0, 2.3].into_iter().enumerate() {
        let move_point = if index == 0 || preserve_vertices {
            start
        } else {
            Point {
                x: start.x + random_offset(2.0, roughness, 1.0, random),
                y: start.y + random_offset(2.0, roughness, 1.0, random),
            }
        };
        operations.push(PathOperation::Move(move_point));
        let endpoint = if preserve_vertices {
            end
        } else {
            Point {
                x: end.x + random_offset(extent, roughness, 1.0, random),
                y: end.y + random_offset(extent, roughness, 1.0, random),
            }
        };
        operations.push(PathOperation::Cubic {
            control_1: Point {
                x: control_1.x + random_offset(extent, roughness, 1.0, random),
                y: control_1.y + random_offset(extent, roughness, 1.0, random),
            },
            control_2: Point {
                x: control_2.x + random_offset(extent, roughness, 1.0, random),
                y: control_2.y + random_offset(extent, roughness, 1.0, random),
            },
            end: endpoint,
        });
    }
}

fn random_range(minimum: f64, maximum: f64, roughness: f64, random: &mut Random) -> f64 {
    roughness * (random.next() * (maximum - minimum) + minimum)
}

fn double_line(
    start: Point,
    end: Point,
    roughness: f64,
    preserve_vertices: bool,
    random: &mut Random,
    operations: &mut Vec<PathOperation>,
) {
    line(
        start,
        end,
        roughness,
        preserve_vertices,
        false,
        random,
        operations,
    );
    line(
        start,
        end,
        roughness,
        preserve_vertices,
        true,
        random,
        operations,
    );
}

fn line(
    start: Point,
    end: Point,
    roughness: f64,
    preserve_vertices: bool,
    overlay: bool,
    random: &mut Random,
    operations: &mut Vec<PathOperation>,
) {
    const MAX_RANDOMNESS_OFFSET: f64 = 2.0;
    const BOWING: f64 = 1.0;

    let delta_x = end.x - start.x;
    let delta_y = end.y - start.y;
    let distance_x = start.x - end.x;
    let distance_y = start.y - end.y;
    let length_squared = distance_x * distance_x + distance_y * distance_y;
    let length = length_squared.sqrt();
    let roughness_gain = if length < 200.0 {
        1.0
    } else if length > 500.0 {
        0.4
    } else {
        -0.0016668 * length + 1.233334
    };
    let mut randomness_offset = MAX_RANDOMNESS_OFFSET;
    if randomness_offset * randomness_offset * 100.0 > length_squared {
        randomness_offset = length / 10.0;
    }
    let half_offset = randomness_offset / 2.0;
    let diverge_point = 0.2 + random.next() * 0.2;

    let mut mid_displacement_x = BOWING * MAX_RANDOMNESS_OFFSET;
    mid_displacement_x *= delta_y;
    mid_displacement_x /= 200.0;
    let mut mid_displacement_y = BOWING * MAX_RANDOMNESS_OFFSET;
    mid_displacement_y *= -delta_x;
    mid_displacement_y /= 200.0;
    mid_displacement_x = random_offset(mid_displacement_x, roughness, roughness_gain, random);
    mid_displacement_y = random_offset(mid_displacement_y, roughness, roughness_gain, random);

    let offset = if overlay {
        half_offset
    } else {
        randomness_offset
    };
    let moved_start = if preserve_vertices {
        start
    } else {
        Point {
            x: start.x + random_offset(offset, roughness, roughness_gain, random),
            y: start.y + random_offset(offset, roughness, roughness_gain, random),
        }
    };
    operations.push(PathOperation::Move(moved_start));
    let control_1 = Point {
        x: control_coordinate(
            mid_displacement_x,
            start.x,
            delta_x,
            diverge_point,
            offset,
            roughness,
            roughness_gain,
            random,
        ),
        y: control_coordinate(
            mid_displacement_y,
            start.y,
            delta_y,
            diverge_point,
            offset,
            roughness,
            roughness_gain,
            random,
        ),
    };
    let doubled_delta_x = 2.0 * delta_x;
    let doubled_delta_y = 2.0 * delta_y;
    let control_2 = Point {
        x: control_coordinate(
            mid_displacement_x,
            start.x,
            doubled_delta_x,
            diverge_point,
            offset,
            roughness,
            roughness_gain,
            random,
        ),
        y: control_coordinate(
            mid_displacement_y,
            start.y,
            doubled_delta_y,
            diverge_point,
            offset,
            roughness,
            roughness_gain,
            random,
        ),
    };
    let endpoint = if preserve_vertices {
        end
    } else {
        Point {
            x: end.x + random_offset(offset, roughness, roughness_gain, random),
            y: end.y + random_offset(offset, roughness, roughness_gain, random),
        }
    };
    operations.push(PathOperation::Cubic {
        control_1,
        control_2,
        end: endpoint,
    });
}

#[allow(clippy::too_many_arguments)]
fn control_coordinate(
    middle_displacement: f64,
    start: f64,
    delta: f64,
    diverge_point: f64,
    randomness_offset: f64,
    roughness: f64,
    roughness_gain: f64,
    random: &mut Random,
) -> f64 {
    let mut coordinate = middle_displacement + start;
    coordinate += delta * diverge_point;
    coordinate + random_offset(randomness_offset, roughness, roughness_gain, random)
}

fn random_offset(extent: f64, roughness: f64, roughness_gain: f64, random: &mut Random) -> f64 {
    let minimum = -extent;
    let maximum = extent;
    let span = maximum - minimum;
    let shifted = random.next() * span + minimum;
    let scaled_roughness = roughness * roughness_gain;
    scaled_roughness * shifted
}

struct Random {
    seed: i32,
}

impl Random {
    const fn new(seed: i32) -> Self {
        Self { seed }
    }

    fn next(&mut self) -> f64 {
        self.seed = 48_271_i32.wrapping_mul(self.seed);
        f64::from((self.seed as u32) & 0x7fff_ffff) / 2_147_483_648.0
    }
}
// Focused port of Rough.js 4.6.4 rectangle/line generation.
// Copyright (c) 2019 Preet Shihn, licensed under MIT. See
// `crates/excalidraw-render/LICENSES/roughjs-MIT.txt`.
