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

pub(super) fn rectangle(
    width: f64,
    height: f64,
    seed: i32,
    roughness: f64,
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
        double_line(start, end, roughness, &mut random, &mut operations);
    }
    Ok(Path { operations })
}

fn double_line(
    start: Point,
    end: Point,
    roughness: f64,
    random: &mut Random,
    operations: &mut Vec<PathOperation>,
) {
    line(start, end, roughness, false, random, operations);
    line(start, end, roughness, true, random, operations);
}

fn line(
    start: Point,
    end: Point,
    roughness: f64,
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

    operations.push(PathOperation::Move(start));
    let offset = if overlay {
        half_offset
    } else {
        randomness_offset
    };
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
    operations.push(PathOperation::Cubic {
        control_1,
        control_2,
        end,
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

use std::collections::TryReserveError;
