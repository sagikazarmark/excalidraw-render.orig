//! Exact pass/fail behavior of Mapbox Pixelmatch 7.2.0.

use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub threshold: f64,
    pub include_aa: bool,
    pub checkerboard: bool,
    pub alpha: f64,
    pub aa_color: [u8; 3],
    pub diff_color: [u8; 3],
    pub diff_color_alt: Option<[u8; 3]>,
    pub diff_mask: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            threshold: f64::from_bits(0x3fb9_9999_9999_999a),
            include_aa: false,
            checkerboard: true,
            alpha: 0.1,
            aa_color: [255, 255, 0],
            diff_color: [255, 0, 0],
            diff_color_alt: None,
            diff_mask: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PixelmatchResult {
    pub changed_mask: Vec<bool>,
    pub diff_rgba: Vec<u8>,
}

impl PixelmatchResult {
    pub fn mismatch_count(&self) -> usize {
        self.changed_mask.iter().filter(|changed| **changed).count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PixelmatchError {
    ZeroDimension,
    DimensionsTooLarge,
    InvalidBufferLength {
        image: usize,
        expected: usize,
        actual: usize,
    },
    InvalidThreshold,
    InvalidDiagnosticAlpha,
}

impl fmt::Display for PixelmatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDimension => formatter.write_str("image dimensions must be nonzero"),
            Self::DimensionsTooLarge => formatter.write_str("image dimensions are too large"),
            Self::InvalidBufferLength {
                image,
                expected,
                actual,
            } => write!(
                formatter,
                "image {image} must contain {expected} RGBA8 bytes, got {actual}"
            ),
            Self::InvalidThreshold => {
                formatter.write_str("Pixelmatch threshold must be finite and within 0..=1")
            }
            Self::InvalidDiagnosticAlpha => {
                formatter.write_str("Pixelmatch diagnostic alpha must be finite and within 0..=1")
            }
        }
    }
}

impl Error for PixelmatchError {}

pub fn compare(
    first: &[u8],
    second: &[u8],
    width: u32,
    height: u32,
    options: Options,
) -> Result<PixelmatchResult, PixelmatchError> {
    if width == 0 || height == 0 {
        return Err(PixelmatchError::ZeroDimension);
    }
    if !options.threshold.is_finite() || !(0.0..=1.0).contains(&options.threshold) {
        return Err(PixelmatchError::InvalidThreshold);
    }
    if !options.alpha.is_finite() || !(0.0..=1.0).contains(&options.alpha) {
        return Err(PixelmatchError::InvalidDiagnosticAlpha);
    }

    let width = usize::try_from(width).map_err(|_| PixelmatchError::DimensionsTooLarge)?;
    let height = usize::try_from(height).map_err(|_| PixelmatchError::DimensionsTooLarge)?;
    let pixel_count = width
        .checked_mul(height)
        .ok_or(PixelmatchError::DimensionsTooLarge)?;
    let expected = pixel_count
        .checked_mul(4)
        .ok_or(PixelmatchError::DimensionsTooLarge)?;
    for (index, image) in [first, second].into_iter().enumerate() {
        if image.len() != expected {
            return Err(PixelmatchError::InvalidBufferLength {
                image: index + 1,
                expected,
                actual: image.len(),
            });
        }
    }

    let mut result = PixelmatchResult {
        changed_mask: vec![false; pixel_count],
        diff_rgba: vec![0; expected],
    };
    if first == second {
        if !options.diff_mask {
            for position in (0..expected).step_by(4) {
                draw_gray_pixel(first, position, options.alpha, &mut result.diff_rgba);
            }
        }
        return Ok(result);
    }

    let max_delta = 35215.0 * options.threshold;
    let max_delta = max_delta * options.threshold;
    for index in 0..pixel_count {
        let position = index * 4;
        let delta = if first[position..position + 4] == second[position..position + 4] {
            0.0
        } else {
            color_delta(first, second, position, position, options.checkerboard)
        };

        if delta.abs() > max_delta {
            let x = index % width;
            let y = index / width;
            let excluded_aa = !options.include_aa
                && (antialiased(
                    first,
                    x,
                    y,
                    width,
                    height,
                    first,
                    second,
                    options.checkerboard,
                ) || antialiased(
                    second,
                    x,
                    y,
                    width,
                    height,
                    second,
                    first,
                    options.checkerboard,
                ));
            if excluded_aa {
                if !options.diff_mask {
                    draw_pixel(&mut result.diff_rgba, position, options.aa_color);
                }
            } else {
                result.changed_mask[index] = true;
                let color = if delta < 0.0 {
                    options.diff_color_alt.unwrap_or(options.diff_color)
                } else {
                    options.diff_color
                };
                draw_pixel(&mut result.diff_rgba, position, color);
            }
        } else if !options.diff_mask {
            draw_gray_pixel(first, position, options.alpha, &mut result.diff_rgba);
        }
    }

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn antialiased(
    image: &[u8],
    center_x: usize,
    center_y: usize,
    width: usize,
    height: usize,
    first_pixels: &[u8],
    second_pixels: &[u8],
    checkerboard: bool,
) -> bool {
    let x0 = center_x.saturating_sub(1);
    let y0 = center_y.saturating_sub(1);
    let x2 = (center_x + 1).min(width - 1);
    let y2 = (center_y + 1).min(height - 1);
    let center_position = (center_y * width + center_x) * 4;
    let center = [
        image[center_position],
        image[center_position + 1],
        image[center_position + 2],
        image[center_position + 3],
    ];
    let mut equal_siblings =
        usize::from(center_x == x0 || center_x == x2 || center_y == y0 || center_y == y2);
    let mut minimum = 0.0;
    let mut maximum = 0.0;
    let mut minimum_coordinate = (0, 0);
    let mut maximum_coordinate = (0, 0);

    for x in x0..=x2 {
        for y in y0..=y2 {
            if x == center_x && y == center_y {
                continue;
            }
            let delta = brightness_delta(
                image,
                center_position,
                (y * width + x) * 4,
                center,
                checkerboard,
            );
            if delta == 0.0 {
                equal_siblings += 1;
                if equal_siblings > 2 {
                    return false;
                }
            } else if delta < minimum {
                minimum = delta;
                minimum_coordinate = (x, y);
            } else if delta > maximum {
                maximum = delta;
                maximum_coordinate = (x, y);
            }
        }
    }

    if minimum == 0.0 || maximum == 0.0 {
        return false;
    }
    (has_many_siblings(first_pixels, minimum_coordinate, width, height)
        && has_many_siblings(second_pixels, minimum_coordinate, width, height))
        || (has_many_siblings(first_pixels, maximum_coordinate, width, height)
            && has_many_siblings(second_pixels, maximum_coordinate, width, height))
}

fn has_many_siblings(
    image: &[u8],
    coordinate: (usize, usize),
    width: usize,
    height: usize,
) -> bool {
    let (center_x, center_y) = coordinate;
    let x0 = center_x.saturating_sub(1);
    let y0 = center_y.saturating_sub(1);
    let x2 = (center_x + 1).min(width - 1);
    let y2 = (center_y + 1).min(height - 1);
    let center_position = (center_y * width + center_x) * 4;
    let center = &image[center_position..center_position + 4];
    let mut equal_siblings =
        usize::from(center_x == x0 || center_x == x2 || center_y == y0 || center_y == y2);

    for x in x0..=x2 {
        for y in y0..=y2 {
            if x == center_x && y == center_y {
                continue;
            }
            let position = (y * width + x) * 4;
            equal_siblings += usize::from(center == &image[position..position + 4]);
            if equal_siblings > 2 {
                return true;
            }
        }
    }
    false
}

fn color_delta(
    first: &[u8],
    second: &[u8],
    first_position: usize,
    second_position: usize,
    checkerboard: bool,
) -> f64 {
    let first_pixel = pixel_at(first, first_position);
    let second_pixel = pixel_at(second, second_position);
    let [red_delta, green_delta, blue_delta] =
        channel_deltas(first_pixel, second_pixel, first_position, checkerboard);

    let y = red_delta * 0.29889531 + green_delta * 0.58662247;
    let y = y + blue_delta * 0.11448223;
    let i = red_delta * 0.59597799 - green_delta * 0.27417610;
    let i = i - blue_delta * 0.32180189;
    let q = red_delta * 0.21147017 - green_delta * 0.52261711;
    let q = q + blue_delta * 0.31114694;

    let delta_y = 0.5053 * y;
    let delta_y = delta_y * y;
    let delta_i = 0.299 * i;
    let delta_i = delta_i * i;
    let delta_q = 0.1957 * q;
    let delta_q = delta_q * q;
    let delta = delta_y + delta_i;
    let delta = delta + delta_q;
    if y > 0.0 {
        -delta
    } else {
        delta
    }
}

fn brightness_delta(
    image: &[u8],
    center_position: usize,
    neighbor_position: usize,
    center: [u8; 4],
    checkerboard: bool,
) -> f64 {
    let neighbor = &image[neighbor_position..neighbor_position + 4];
    let [red_delta, green_delta, blue_delta, alpha_delta] = [
        f64::from(center[0]) - f64::from(neighbor[0]),
        f64::from(center[1]) - f64::from(neighbor[1]),
        f64::from(center[2]) - f64::from(neighbor[2]),
        f64::from(center[3]) - f64::from(neighbor[3]),
    ];
    if red_delta == 0.0 && green_delta == 0.0 && blue_delta == 0.0 && alpha_delta == 0.0 {
        return 0.0;
    }
    let [red_delta, green_delta, blue_delta] = channel_deltas(
        center.map(f64::from),
        pixel_at(image, neighbor_position),
        center_position,
        checkerboard,
    );

    let delta = red_delta * 0.29889531 + green_delta * 0.58662247;
    delta + blue_delta * 0.11448223
}

fn pixel_at(image: &[u8], position: usize) -> [f64; 4] {
    [
        f64::from(image[position]),
        f64::from(image[position + 1]),
        f64::from(image[position + 2]),
        f64::from(image[position + 3]),
    ]
}

fn channel_deltas(
    first: [f64; 4],
    second: [f64; 4],
    background_position: usize,
    checkerboard: bool,
) -> [f64; 3] {
    let [red1, green1, blue1, alpha1] = first;
    let [red2, green2, blue2, alpha2] = second;
    let mut red_delta = red1 - red2;
    let mut green_delta = green1 - green2;
    let mut blue_delta = blue1 - blue2;
    let alpha_delta = alpha1 - alpha2;
    if alpha1 < 255.0 || alpha2 < 255.0 {
        let background = background_at(background_position, checkerboard);
        let red = red1 * alpha1 - red2 * alpha2;
        red_delta = (red - background[0] * alpha_delta) / 255.0;
        let green = green1 * alpha1 - green2 * alpha2;
        green_delta = (green - background[1] * alpha_delta) / 255.0;
        let blue = blue1 * alpha1 - blue2 * alpha2;
        blue_delta = (blue - background[2] * alpha_delta) / 255.0;
    }
    [red_delta, green_delta, blue_delta]
}

fn background_at(position: usize, checkerboard: bool) -> [f64; 3] {
    if !checkerboard {
        return [255.0; 3];
    }
    let position = position as f64;
    let red_index = (position as usize % 2) as i32;
    let green_index = to_int32(position / 1.618033988749895) % 2;
    let blue_index = to_int32(position / 2.618033988749895) % 2;
    [red_index, green_index, blue_index].map(|index| f64::from(48 + 159 * index))
}

fn to_int32(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let integer = value.trunc();
    let int32_bits = integer % 4_294_967_296.0;
    let int32_bits = if int32_bits < 0.0 {
        int32_bits + 4_294_967_296.0
    } else {
        int32_bits
    };
    if int32_bits >= 2_147_483_648.0 {
        (int32_bits - 4_294_967_296.0) as i32
    } else {
        int32_bits as i32
    }
}

fn draw_pixel(output: &mut [u8], position: usize, color: [u8; 3]) {
    output[position..position + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
}

fn draw_gray_pixel(image: &[u8], position: usize, alpha: f64, output: &mut [u8]) {
    let luma =
        f64::from(image[position]) * 0.29889531 + f64::from(image[position + 1]) * 0.58662247;
    let luma = luma + f64::from(image[position + 2]) * 0.11448223;
    let value = 255.0 + (luma - 255.0) * alpha * f64::from(image[position + 3]) / 255.0;
    let value = value as u8;
    draw_pixel(output, position, [value; 3]);
}
