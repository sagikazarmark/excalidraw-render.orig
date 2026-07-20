use crate::RenderRequest;
use tiny_skia::{
    BlendMode, Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, PathStroker, PixmapMut,
    Shader, Stroke, Transform,
};

use super::{
    rough::{Path, PathOperation, Point},
    FormatEncodeError, NormalizedElementSource, ResolvedElement, ResolvedScene,
};

pub(super) struct PngPlan {
    width: u32,
    height: u32,
    background: Option<[u8; 4]>,
    elements: Vec<PngElement>,
}

enum PngElement {
    RoughRectangle {
        source: NormalizedElementSource,
        path: Path,
        transform: PngTransform,
        stroke_color: [u8; 3],
        stroke_width: f64,
        opacity: f64,
        base_element_work_units: u64,
        render: bool,
    },
}

#[derive(Clone, Copy)]
struct PngTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
    scale: f64,
}

impl PngTransform {
    fn new(transform: &super::ResolvedTransform, scale: f64) -> Self {
        let sin = transform.angle_radians.sin();
        let cos = transform.angle_radians.cos();
        Self {
            a: scale * cos,
            b: scale * sin,
            c: -scale * sin,
            d: scale * cos,
            e: scale
                * (transform.translation_x + transform.center_x - cos * transform.center_x
                    + sin * transform.center_y),
            f: scale
                * (transform.translation_y + transform.center_y
                    - sin * transform.center_x
                    - cos * transform.center_y),
            scale,
        }
    }

    fn raster_transform(self, multiplier: u32) -> Result<Transform, FormatEncodeError> {
        let multiplier = f64::from(multiplier);
        let transform = Transform::from_row(
            finite_f32(self.a * multiplier, "PNG transform a")?,
            finite_f32(self.b * multiplier, "PNG transform b")?,
            finite_f32(self.c * multiplier, "PNG transform c")?,
            finite_f32(self.d * multiplier, "PNG transform d")?,
            finite_f32(self.e * multiplier, "PNG transform e")?,
            finite_f32(self.f * multiplier, "PNG transform f")?,
        );
        if !transform.is_valid() {
            return Err(FormatEncodeError::Encoding(
                "PNG transform is not invertible".to_owned(),
            ));
        }
        Ok(transform)
    }

    fn map_point(self, point: Point) -> Point {
        Point {
            x: self.a * point.x + self.c * point.y + self.e,
            y: self.b * point.x + self.d * point.y + self.f,
        }
    }
}

impl PngPlan {
    pub(super) fn new(
        scene: ResolvedScene,
        request: &RenderRequest,
    ) -> Result<Self, FormatEncodeError> {
        let scale = scene.raster_or_pdf_scale(request.sizing());
        let width = pixel_dimension(scene.logical_width * scale)?;
        let height = pixel_dimension(scene.logical_height * scale)?;
        let background = scene
            .include_background
            .then(|| {
                parse_hex_rgb(&scene.background_color).map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            })
            .transpose()?;
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(scene.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "png_elements",
            })?;
        for element in scene.elements {
            match element {
                ResolvedElement::Rectangle(rectangle) => {
                    if rectangle.source.id().is_empty() {
                        return Err(FormatEncodeError::Encoding(format!(
                            "rectangle at {} has no normalized identity",
                            rectangle.source.pointer()
                        )));
                    }
                    elements.push(PngElement::RoughRectangle {
                        source: rectangle.source,
                        path: rectangle.path,
                        transform: PngTransform::new(&rectangle.transform, scale),
                        stroke_color: parse_hex_rgb(&rectangle.stroke_color)?,
                        stroke_width: rectangle.stroke_width,
                        opacity: rectangle.opacity,
                        base_element_work_units: rectangle.base_element_work_units,
                        render: true,
                    });
                }
            }
        }
        Ok(Self {
            width,
            height,
            background,
            elements,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let mut rgba = self.rasterize()?;
        canonicalize_straight_alpha(&mut rgba)?;
        encode_rgba(self.width, self.height, &rgba)
    }

    fn rasterize(&self) -> Result<Vec<u8>, FormatEncodeError> {
        // Fixed supersampling matches browser-canvas edge coverage without host raster defaults.
        const SUBPIXEL_SCALE: u32 = 4;

        let raster_width = self
            .width
            .checked_mul(SUBPIXEL_SCALE)
            .ok_or_else(png_size_error)?;
        let raster_height = self
            .height
            .checked_mul(SUBPIXEL_SCALE)
            .ok_or_else(png_size_error)?;
        let rgba_bytes = usize::try_from(raster_width)
            .ok()
            .and_then(|width| width.checked_mul(usize::try_from(raster_height).ok()?))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(png_size_error)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(rgba_bytes)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "raster_surface",
                subject: "png_main_surface",
            })?;
        rgba.resize(rgba_bytes, 0);

        {
            let mut surface = PixmapMut::from_bytes(&mut rgba, raster_width, raster_height)
                .ok_or_else(png_size_error)?;
            if let Some([red, green, blue, alpha]) = self.background {
                surface.fill(Color::from_rgba8(red, green, blue, alpha));
            }
            for element in &self.elements {
                match element {
                    PngElement::RoughRectangle {
                        source,
                        path,
                        transform,
                        stroke_color,
                        stroke_width,
                        opacity,
                        render,
                        ..
                    } => {
                        if !render {
                            continue;
                        }
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG plan rectangle lost its source identity".to_owned(),
                            ));
                        }
                        let path = tiny_skia_path(path)?;
                        let transform = transform.raster_transform(SUBPIXEL_SCALE)?;
                        let color = Color::from_rgba(
                            f32::from(stroke_color[0]) / 255.0,
                            f32::from(stroke_color[1]) / 255.0,
                            f32::from(stroke_color[2]) / 255.0,
                            finite_f32(*opacity, "PNG stroke opacity")?,
                        )
                        .ok_or_else(|| {
                            FormatEncodeError::Encoding(
                                "PNG stroke color is outside RGBA range".to_owned(),
                            )
                        })?;
                        let paint = Paint {
                            shader: Shader::SolidColor(color),
                            blend_mode: BlendMode::SourceOver,
                            anti_alias: true,
                            force_hq_pipeline: true,
                        };
                        let stroke = Stroke {
                            width: finite_f32(*stroke_width, "PNG stroke width")?,
                            miter_limit: 4.0,
                            line_cap: LineCap::Round,
                            line_join: LineJoin::Miter,
                            dash: None,
                        };
                        let resolution_scale = PathStroker::compute_resolution_scale(&transform);
                        let path = path.stroke(&stroke, resolution_scale).ok_or_else(|| {
                            FormatEncodeError::Encoding(
                                "PNG raster adapter could not stroke the Rough path".to_owned(),
                            )
                        })?;
                        let path = path.transform(transform).ok_or_else(|| {
                            FormatEncodeError::Encoding(
                                "PNG raster adapter could not transform the stroked path"
                                    .to_owned(),
                            )
                        })?;
                        let bounds = path.bounds();
                        if !(bounds.width() > 0.0 && bounds.height() > 0.0) {
                            return Err(FormatEncodeError::Encoding(
                                "PNG raster adapter produced empty stroked bounds".to_owned(),
                            ));
                        }
                        surface.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
        }
        downsample_rgba(&rgba, raster_width, self.width, self.height, SUBPIXEL_SCALE)
    }

    pub(super) fn vector_coverage_area(&self, index: usize) -> Option<u64> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughRectangle {
                path,
                transform,
                stroke_width,
                ..
            } => {
                conservative_coverage_area(path, *transform, *stroke_width, self.width, self.height)
            }
        })
    }

    pub(super) fn base_element_work_units(&self, index: usize) -> Option<u64> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughRectangle {
                base_element_work_units,
                ..
            } => *base_element_work_units,
        })
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(&str, &str)> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughRectangle { source, .. } => (source.id(), source.pointer()),
        })
    }

    pub(super) fn omit_element(&mut self, index: usize) -> bool {
        match self.elements.get_mut(index) {
            Some(PngElement::RoughRectangle { render, .. }) => {
                *render = false;
                true
            }
            None => false,
        }
    }

    pub(super) const fn includes_background(&self) -> bool {
        self.background.is_some()
    }

    pub(super) const fn width(&self) -> u32 {
        self.width
    }

    pub(super) const fn height(&self) -> u32 {
        self.height
    }
}

fn downsample_rgba(
    source: &[u8],
    source_width: u32,
    width: u32,
    height: u32,
    scale: u32,
) -> Result<Vec<u8>, FormatEncodeError> {
    let output_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(usize::try_from(height).ok()?))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(png_size_error)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_bytes)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "raster_surface",
            subject: "png_downsampled_surface",
        })?;
    output.resize(output_bytes, 0);
    let sample_count = scale.saturating_mul(scale);
    for y in 0..height {
        for x in 0..width {
            let mut sums = [0_u32; 4];
            for sample_y in 0..scale {
                for sample_x in 0..scale {
                    let source_pixel = u64::from(y * scale + sample_y)
                        .saturating_mul(u64::from(source_width))
                        .saturating_add(u64::from(x * scale + sample_x));
                    let source_offset = usize::try_from(source_pixel.saturating_mul(4))
                        .map_err(|_| png_size_error())?;
                    let source_end = source_offset.checked_add(4).ok_or_else(png_size_error)?;
                    let sample = source.get(source_offset..source_end).ok_or_else(|| {
                        FormatEncodeError::Encoding(
                            "PNG subpixel surface length invariant failed".to_owned(),
                        )
                    })?;
                    for (sum, channel) in sums.iter_mut().zip(sample) {
                        *sum += u32::from(*channel);
                    }
                }
            }
            let output_pixel = u64::from(y)
                .saturating_mul(u64::from(width))
                .saturating_add(u64::from(x));
            let output_offset =
                usize::try_from(output_pixel.saturating_mul(4)).map_err(|_| png_size_error())?;
            let output_end = output_offset.checked_add(4).ok_or_else(png_size_error)?;
            let pixel = output.get_mut(output_offset..output_end).ok_or_else(|| {
                FormatEncodeError::Encoding(
                    "PNG downsampled surface length invariant failed".to_owned(),
                )
            })?;
            for (channel, sum) in pixel.iter_mut().zip(sums) {
                *channel = u8::try_from((sum + sample_count / 2) / sample_count).map_err(|_| {
                    FormatEncodeError::Encoding(
                        "PNG downsampled channel invariant failed".to_owned(),
                    )
                })?;
            }
        }
    }
    Ok(output)
}

fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, FormatEncodeError> {
    const MAX_STORED_BLOCK_BYTES: usize = u16::MAX as usize;

    // Stored DEFLATE makes the exact PNG size known before fallible output allocation.
    let pixel_row_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(png_size_error)?;
    let row_bytes = pixel_row_bytes.checked_add(1).ok_or_else(png_size_error)?;
    let raw_bytes = row_bytes
        .checked_mul(usize::try_from(height).map_err(|_| png_size_error())?)
        .ok_or_else(png_size_error)?;
    let expected_rgba_bytes = pixel_row_bytes
        .checked_mul(usize::try_from(height).map_err(|_| png_size_error())?)
        .ok_or_else(png_size_error)?;
    if rgba.len() != expected_rgba_bytes {
        return Err(FormatEncodeError::Encoding(
            "PNG raster length does not match its dimensions".to_owned(),
        ));
    }
    let block_count = raw_bytes.div_ceil(MAX_STORED_BLOCK_BYTES);
    let zlib_bytes = 2_usize
        .checked_add(block_count.checked_mul(5).ok_or_else(png_size_error)?)
        .and_then(|bytes| bytes.checked_add(raw_bytes))
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or_else(png_size_error)?;
    let output_bytes = 8_usize
        .checked_add(12 + 13)
        .and_then(|bytes| bytes.checked_add(12))
        .and_then(|bytes| bytes.checked_add(zlib_bytes))
        .and_then(|bytes| bytes.checked_add(12))
        .ok_or_else(png_size_error)?;
    let idat_bytes = u32::try_from(zlib_bytes).map_err(|_| png_size_error())?;

    let mut png = Vec::new();
    png.try_reserve_exact(output_bytes)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "artifact",
            subject: "png_output",
        })?;
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = [0_u8; 13];
    ihdr[..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8] = 8;
    ihdr[9] = 6;
    append_chunk(&mut png, *b"IHDR", &ihdr)?;

    png.extend_from_slice(&idat_bytes.to_be_bytes());
    let idat_start = png.len();
    png.extend_from_slice(b"IDAT");
    png.extend_from_slice(&[0x78, 0x01]);
    let mut adler_a = 1_u32;
    let mut adler_b = 0_u32;
    let mut offset = 0_usize;
    while offset < raw_bytes {
        let block_bytes = (raw_bytes - offset).min(MAX_STORED_BLOCK_BYTES);
        png.push(u8::from(offset + block_bytes == raw_bytes));
        let block_bytes = u16::try_from(block_bytes).map_err(|_| png_size_error())?;
        png.extend_from_slice(&block_bytes.to_le_bytes());
        png.extend_from_slice(&(!block_bytes).to_le_bytes());
        for raw_offset in offset..offset + usize::from(block_bytes) {
            let row_offset = raw_offset % row_bytes;
            let byte = if row_offset == 0 {
                0
            } else {
                let row = raw_offset / row_bytes;
                let rgba_offset = row
                    .checked_mul(pixel_row_bytes)
                    .and_then(|offset| offset.checked_add(row_offset - 1))
                    .ok_or_else(png_size_error)?;
                *rgba.get(rgba_offset).ok_or_else(|| {
                    FormatEncodeError::Encoding(
                        "PNG encoder raster offset invariant failed".to_owned(),
                    )
                })?
            };
            png.push(byte);
            adler_a = (adler_a + u32::from(byte)) % 65_521;
            adler_b = (adler_b + adler_a) % 65_521;
        }
        offset += usize::from(block_bytes);
    }
    png.extend_from_slice(&((adler_b << 16) | adler_a).to_be_bytes());
    let idat_crc = crc32(&png[idat_start..]);
    png.extend_from_slice(&idat_crc.to_be_bytes());
    append_chunk(&mut png, *b"IEND", &[])?;

    if png.len() != output_bytes {
        return Err(FormatEncodeError::Encoding(
            "PNG encoder length invariant failed".to_owned(),
        ));
    }
    Ok(png)
}

fn tiny_skia_path(path: &Path) -> Result<tiny_skia::Path, FormatEncodeError> {
    let points = path
        .operations
        .len()
        .checked_mul(3)
        .ok_or_else(png_size_error)?;
    let mut builder = PathBuilder::with_capacity(path.operations.len(), points);
    for operation in &path.operations {
        match operation {
            PathOperation::Move(point) => builder.move_to(
                finite_f32(point.x, "PNG path x")?,
                finite_f32(point.y, "PNG path y")?,
            ),
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => builder.cubic_to(
                finite_f32(control_1.x, "PNG control x")?,
                finite_f32(control_1.y, "PNG control y")?,
                finite_f32(control_2.x, "PNG control x")?,
                finite_f32(control_2.y, "PNG control y")?,
                finite_f32(end.x, "PNG path x")?,
                finite_f32(end.y, "PNG path y")?,
            ),
        }
    }
    builder.finish().ok_or_else(|| {
        FormatEncodeError::Encoding("PNG path contains no drawable operations".to_owned())
    })
}

fn conservative_coverage_area(
    path: &Path,
    transform: PngTransform,
    stroke_width: f64,
    surface_width: u32,
    surface_height: u32,
) -> u64 {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut include = |point| {
        let point = transform.map_point(point);
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    };
    for operation in &path.operations {
        match operation {
            PathOperation::Move(point) => include(*point),
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                for point in [*control_1, *control_2, *end] {
                    include(point);
                }
            }
        }
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return 0;
    }
    let outset = stroke_width * transform.scale / 2.0;
    let left = (min_x - outset)
        .floor()
        .clamp(0.0, f64::from(surface_width));
    let top = (min_y - outset)
        .floor()
        .clamp(0.0, f64::from(surface_height));
    let right = (max_x + outset).ceil().clamp(0.0, f64::from(surface_width));
    let bottom = (max_y + outset)
        .ceil()
        .clamp(0.0, f64::from(surface_height));
    ((right - left) as u64).saturating_mul((bottom - top) as u64)
}

fn canonicalize_straight_alpha(rgba: &mut [u8]) -> Result<(), FormatEncodeError> {
    for pixel in rgba.chunks_exact_mut(4) {
        let (channels, alpha) = pixel.split_at_mut(3);
        let alpha = alpha.first().copied().ok_or_else(|| {
            FormatEncodeError::Encoding("PNG alpha layout invariant failed".to_owned())
        })?;
        let alpha = u32::from(alpha);
        if alpha == 0 {
            channels.fill(0);
            continue;
        }
        for channel in channels {
            let straight = (u32::from(*channel) * 255 + alpha / 2) / alpha;
            *channel = u8::try_from(straight.min(255)).map_err(|_| {
                FormatEncodeError::Encoding(
                    "PNG straight-alpha channel invariant failed".to_owned(),
                )
            })?;
        }
    }
    Ok(())
}

fn parse_hex_rgb(value: &str) -> Result<[u8; 3], FormatEncodeError> {
    let [b'#', red_high, red_low, green_high, green_low, blue_high, blue_low] = value.as_bytes()
    else {
        return Err(FormatEncodeError::Encoding(
            "PNG plan color is not canonical six-digit sRGB".to_owned(),
        ));
    };
    Ok([
        parse_hex_byte(*red_high, *red_low)?,
        parse_hex_byte(*green_high, *green_low)?,
        parse_hex_byte(*blue_high, *blue_low)?,
    ])
}

fn parse_hex_byte(high: u8, low: u8) -> Result<u8, FormatEncodeError> {
    let high = hex_nibble(high).ok_or_else(invalid_png_color)?;
    let low = hex_nibble(low).ok_or_else(invalid_png_color)?;
    Ok((high << 4) | low)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn invalid_png_color() -> FormatEncodeError {
    FormatEncodeError::Encoding("PNG plan color is not canonical six-digit sRGB".to_owned())
}

fn finite_f32(value: f64, subject: &str) -> Result<f32, FormatEncodeError> {
    let narrowed = value as f32;
    if !value.is_finite() || !narrowed.is_finite() {
        return Err(FormatEncodeError::Encoding(format!(
            "{subject} must be finite in the PNG raster domain"
        )));
    }
    Ok(narrowed)
}

fn append_chunk(output: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) -> Result<(), FormatEncodeError> {
    let length = u32::try_from(data.len()).map_err(|_| png_size_error())?;
    output.extend_from_slice(&length.to_be_bytes());
    let crc_start = output.len();
    output.extend_from_slice(&kind);
    output.extend_from_slice(data);
    output.extend_from_slice(&crc32(&output[crc_start..]).to_be_bytes());
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn png_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("PNG dimensions exceed encoder capacity".to_owned())
}

fn pixel_dimension(value: f64) -> Result<u32, FormatEncodeError> {
    if !value.is_finite() || value < 1.0 || value >= (u32::MAX as f64) + 1.0 {
        return Err(FormatEncodeError::Encoding(
            "PNG dimensions must be finite positive 32-bit values".to_owned(),
        ));
    }
    Ok(value.trunc() as u32)
}
