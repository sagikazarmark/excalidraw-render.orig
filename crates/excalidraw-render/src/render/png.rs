use crate::{color::CanonicalColor, RenderRequest};
use tiny_skia::{
    BlendMode, Color, FillRule, IntSize, LineCap, LineJoin, Mask, Paint, PathBuilder, PathStroker,
    PixmapMut, PixmapPaint, Shader, Stroke, StrokeDash, Transform,
};

use super::{
    rough::{Drawing, FillKind, Path, PathOperation, Point},
    FormatEncodeError, NormalizedElementSource, ResolvedElement, ResolvedScene, StrokeStyle,
};

pub(super) struct PngPlan {
    width: u32,
    height: u32,
    background: Option<[u8; 4]>,
    fallback_colors: super::FallbackColors,
    elements: Vec<PngElement>,
}

enum PngElement {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        transform: PngTransform,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
        background_color: CanonicalColor,
        base_element_work_units: u64,
        render: bool,
    },
    Fallback {
        source: super::FallbackSource,
        label: Option<super::text::ResolvedLabel>,
        frame_clip: Option<PngFrameClip>,
        width: f64,
        height: f64,
        transform: PngTransform,
        opacity: f64,
        base_element_work_units: u64,
        render: bool,
    },
}

#[derive(Clone, Copy)]
struct PngFrameClip {
    width: f64,
    height: f64,
    transform: PngTransform,
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
        self.raster_transform_with_origin(multiplier, 0, 0)
    }

    fn raster_transform_with_origin(
        self,
        multiplier: u32,
        origin_x: u32,
        origin_y: u32,
    ) -> Result<Transform, FormatEncodeError> {
        let multiplier = f64::from(multiplier);
        let transform = Transform::from_row(
            finite_f32(self.a * multiplier, "PNG transform a")?,
            finite_f32(self.b * multiplier, "PNG transform b")?,
            finite_f32(self.c * multiplier, "PNG transform c")?,
            finite_f32(self.d * multiplier, "PNG transform d")?,
            finite_f32(self.e * multiplier - f64::from(origin_x), "PNG transform e")?,
            finite_f32(self.f * multiplier - f64::from(origin_y), "PNG transform f")?,
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
            .then(|| scene.background_color.rgba8());
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(scene.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "png_elements",
            })?;
        for element in scene.elements {
            match element {
                ResolvedElement::Outline(outline) => {
                    if outline.source.id().is_empty() {
                        return Err(FormatEncodeError::Encoding(format!(
                            "outline at {} has no normalized identity",
                            outline.source.pointer()
                        )));
                    }
                    elements.push(PngElement::RoughOutline {
                        source: outline.source,
                        drawing: outline.drawing,
                        transform: PngTransform::new(&outline.transform, scale),
                        stroke_color: outline.stroke_color,
                        fill_color: outline.fill_color,
                        stroke_width: outline.stroke_width,
                        stroke_style: outline.stroke_style,
                        source_stroke_width: outline.source_stroke_width,
                        opacity: outline.opacity,
                        background_color: outline.background_color,
                        base_element_work_units: outline.base_element_work_units,
                        render: true,
                    });
                }
                ResolvedElement::Fallback(fallback) => {
                    let visual_commands: u64 = if fallback.width >= 2.0 && fallback.height >= 2.0 {
                        9
                    } else {
                        5
                    };
                    let label_work = fallback.label.as_ref().map_or(0, |label| {
                        u64::try_from(label.text.chars().count())
                            .unwrap_or(u64::MAX)
                            .saturating_mul(32)
                            .saturating_add(
                                u64::try_from(label.path.operations.len())
                                    .unwrap_or(u64::MAX)
                                    .saturating_mul(32),
                            )
                    });
                    elements.push(PngElement::Fallback {
                        source: fallback.source,
                        label: fallback.label,
                        frame_clip: fallback.frame_clip.map(|clip| PngFrameClip {
                            width: clip.width,
                            height: clip.height,
                            transform: PngTransform::new(&clip.transform, scale),
                        }),
                        width: fallback.width,
                        height: fallback.height,
                        transform: PngTransform::new(&fallback.transform, scale),
                        opacity: fallback.opacity,
                        base_element_work_units: visual_commands * 32 + 8 + label_work,
                        render: true,
                    });
                }
            }
        }
        Ok(Self {
            width,
            height,
            background,
            fallback_colors: scene.fallback_colors,
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
                    PngElement::RoughOutline {
                        source,
                        drawing,
                        transform,
                        stroke_color,
                        fill_color,
                        stroke_width,
                        stroke_style,
                        source_stroke_width,
                        opacity,
                        background_color,
                        render,
                        ..
                    } => {
                        if !render {
                            continue;
                        }
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG plan outline lost its source identity".to_owned(),
                            ));
                        }
                        let transform = transform.raster_transform(SUBPIXEL_SCALE)?;
                        paint_rough_drawing(
                            &mut surface,
                            drawing,
                            transform,
                            *stroke_color,
                            *fill_color,
                            *stroke_width,
                            *stroke_style,
                            *source_stroke_width,
                            *opacity,
                        )?;
                        for decoration in &drawing.decorations {
                            let fill = super::decoration_fill_color(
                                decoration,
                                *stroke_color,
                                *background_color,
                            );
                            let (decoration_style, decoration_stroke_width) =
                                super::decoration_stroke_style(decoration, *source_stroke_width);
                            paint_rough_drawing(
                                &mut surface,
                                &decoration.drawing,
                                transform,
                                *stroke_color,
                                Some(fill),
                                *stroke_width,
                                decoration_style,
                                decoration_stroke_width,
                                *opacity,
                            )?;
                        }
                    }
                    PngElement::Fallback {
                        source,
                        label,
                        frame_clip,
                        width,
                        height,
                        transform,
                        opacity,
                        render,
                        ..
                    } => {
                        if !render {
                            continue;
                        }
                        if source.pointer.is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG fallback lost its source location".to_owned(),
                            ));
                        }
                        let (left, top, right, bottom) = fallback_coverage_bounds(
                            *width,
                            *height,
                            *transform,
                            self.width,
                            self.height,
                        );
                        let layer_width = right
                            .saturating_sub(left)
                            .checked_mul(SUBPIXEL_SCALE)
                            .ok_or_else(png_size_error)?;
                        let layer_height = bottom
                            .saturating_sub(top)
                            .checked_mul(SUBPIXEL_SCALE)
                            .ok_or_else(png_size_error)?;
                        if layer_width == 0 || layer_height == 0 {
                            continue;
                        }
                        let layer_x = left
                            .checked_mul(SUBPIXEL_SCALE)
                            .ok_or_else(png_size_error)?;
                        let layer_y = top.checked_mul(SUBPIXEL_SCALE).ok_or_else(png_size_error)?;
                        let transform = transform.raster_transform_with_origin(
                            SUBPIXEL_SCALE,
                            layer_x,
                            layer_y,
                        )?;
                        let (bounds, cross) = fallback_paths(*width, *height)?;
                        let mask_bytes = usize::try_from(layer_width)
                            .ok()
                            .and_then(|width| {
                                width.checked_mul(usize::try_from(layer_height).ok()?)
                            })
                            .ok_or_else(png_size_error)?;
                        let mut mask_data = Vec::new();
                        mask_data.try_reserve_exact(mask_bytes).map_err(|_| {
                            FormatEncodeError::MemoryUnavailable {
                                subject_kind: "raster_surface",
                                subject: "png_fallback_clip",
                            }
                        })?;
                        mask_data.resize(mask_bytes, 0);
                        let mask_size = IntSize::from_wh(layer_width, layer_height)
                            .ok_or_else(png_size_error)?;
                        let mut clip = Mask::from_vec(mask_data, mask_size).ok_or_else(|| {
                            FormatEncodeError::Encoding(
                                "PNG fallback clip dimensions are invalid".to_owned(),
                            )
                        })?;
                        clip.fill_path(&bounds, FillRule::Winding, true, transform);
                        if let Some(frame_clip) = frame_clip {
                            let (frame_bounds, _) =
                                fallback_paths(frame_clip.width, frame_clip.height)?;
                            clip.intersect_path(
                                &frame_bounds,
                                FillRule::Winding,
                                true,
                                frame_clip.transform.raster_transform_with_origin(
                                    SUBPIXEL_SCALE,
                                    layer_x,
                                    layer_y,
                                )?,
                            );
                        }

                        let layer_rgba_bytes = usize::try_from(layer_width)
                            .ok()
                            .and_then(|width| {
                                width.checked_mul(usize::try_from(layer_height).ok()?)
                            })
                            .and_then(|pixels| pixels.checked_mul(4))
                            .ok_or_else(png_size_error)?;
                        let mut layer_bytes = Vec::new();
                        layer_bytes
                            .try_reserve_exact(layer_rgba_bytes)
                            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                                subject_kind: "raster_surface",
                                subject: "png_fallback_layer",
                            })?;
                        layer_bytes.resize(layer_rgba_bytes, 0);
                        let mut layer =
                            PixmapMut::from_bytes(&mut layer_bytes, layer_width, layer_height)
                                .ok_or_else(png_size_error)?;

                        let fill = solid_paint(self.fallback_colors.fill, 1.0)?;
                        layer.fill_path(&bounds, &fill, FillRule::Winding, transform, Some(&clip));
                        if *width >= 2.0 && *height >= 2.0 {
                            let stroke_paint = solid_paint(self.fallback_colors.stroke, 1.0)?;
                            let dashed_stroke = Stroke {
                                width: 2.0,
                                miter_limit: 4.0,
                                line_cap: LineCap::Butt,
                                line_join: LineJoin::Miter,
                                dash: StrokeDash::new(vec![6.0, 4.0], 0.0),
                            };
                            layer.stroke_path(
                                &bounds,
                                &stroke_paint,
                                &dashed_stroke,
                                transform,
                                Some(&clip),
                            );
                            let cross_stroke = Stroke {
                                width: 2.0,
                                miter_limit: 4.0,
                                line_cap: LineCap::Butt,
                                line_join: LineJoin::Miter,
                                dash: None,
                            };
                            layer.stroke_path(
                                &cross,
                                &stroke_paint,
                                &cross_stroke,
                                transform,
                                Some(&clip),
                            );
                        }
                        if let Some(label) = label {
                            let label_path = label_path(&label.path)?;
                            let label_paint = solid_paint(self.fallback_colors.label, 1.0)?;
                            layer.fill_path(
                                &label_path,
                                &label_paint,
                                FillRule::Winding,
                                transform,
                                Some(&clip),
                            );
                        }
                        let layer_paint = PixmapPaint {
                            opacity: finite_f32(*opacity, "PNG fallback opacity")?,
                            ..PixmapPaint::default()
                        };
                        surface.draw_pixmap(
                            i32::try_from(layer_x).map_err(|_| png_size_error())?,
                            i32::try_from(layer_y).map_err(|_| png_size_error())?,
                            layer.as_ref(),
                            &layer_paint,
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
            PngElement::RoughOutline {
                drawing,
                transform,
                stroke_width,
                ..
            } => conservative_coverage_area(
                drawing,
                *transform,
                *stroke_width,
                self.width,
                self.height,
            ),
            PngElement::Fallback {
                width,
                height,
                transform,
                ..
            } => conservative_fallback_coverage_area(
                *width,
                *height,
                *transform,
                self.width,
                self.height,
            ),
        })
    }

    pub(super) fn base_element_work_units(&self, index: usize) -> Option<u64> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughOutline {
                base_element_work_units,
                ..
            } => *base_element_work_units,
            PngElement::Fallback {
                base_element_work_units,
                ..
            } => *base_element_work_units,
        })
    }

    pub(super) fn fallback_surface_area(&self, index: usize) -> Option<Option<u64>> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughOutline { .. } => None,
            PngElement::Fallback {
                width,
                height,
                transform,
                ..
            } => Some(conservative_fallback_coverage_area(
                *width,
                *height,
                *transform,
                self.width,
                self.height,
            )),
        })
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughOutline { source, .. } => (Some(source.id()), source.pointer()),
            PngElement::Fallback { source, .. } => (source.id.as_deref(), source.pointer.as_str()),
        })
    }

    pub(super) fn omit_element(&mut self, index: usize) -> bool {
        match self.elements.get_mut(index) {
            Some(PngElement::RoughOutline { render, .. }) => {
                *render = false;
                true
            }
            Some(PngElement::Fallback { render, .. }) => {
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
            PathOperation::Line(point) => builder.line_to(
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

fn fallback_paths(
    width: f64,
    height: f64,
) -> Result<(tiny_skia::Path, tiny_skia::Path), FormatEncodeError> {
    let width = finite_f32(width, "PNG fallback width")?;
    let height = finite_f32(height, "PNG fallback height")?;
    let mut bounds = PathBuilder::new();
    bounds.move_to(0.0, 0.0);
    bounds.line_to(width, 0.0);
    bounds.line_to(width, height);
    bounds.line_to(0.0, height);
    bounds.close();
    let bounds = bounds
        .finish()
        .ok_or_else(|| FormatEncodeError::Encoding("PNG fallback bounds are empty".to_owned()))?;
    let mut cross = PathBuilder::new();
    cross.move_to(0.0, 0.0);
    cross.line_to(width, height);
    cross.move_to(width, 0.0);
    cross.line_to(0.0, height);
    let cross = cross
        .finish()
        .ok_or_else(|| FormatEncodeError::Encoding("PNG fallback cross is empty".to_owned()))?;
    Ok((bounds, cross))
}

fn label_path(path: &super::text::LabelPath) -> Result<tiny_skia::Path, FormatEncodeError> {
    let point_capacity = path
        .operations
        .len()
        .checked_mul(3)
        .ok_or_else(png_size_error)?;
    let mut builder = PathBuilder::with_capacity(path.operations.len(), point_capacity);
    for operation in &path.operations {
        match operation {
            super::text::LabelPathOperation::Move { x, y } => builder.move_to(
                finite_f32(*x, "PNG fallback label x")?,
                finite_f32(*y, "PNG fallback label y")?,
            ),
            super::text::LabelPathOperation::Line { x, y } => builder.line_to(
                finite_f32(*x, "PNG fallback label x")?,
                finite_f32(*y, "PNG fallback label y")?,
            ),
            super::text::LabelPathOperation::Quadratic {
                control_x,
                control_y,
                x,
                y,
            } => builder.quad_to(
                finite_f32(*control_x, "PNG fallback label control x")?,
                finite_f32(*control_y, "PNG fallback label control y")?,
                finite_f32(*x, "PNG fallback label x")?,
                finite_f32(*y, "PNG fallback label y")?,
            ),
            super::text::LabelPathOperation::Cubic {
                control_1_x,
                control_1_y,
                control_2_x,
                control_2_y,
                x,
                y,
            } => builder.cubic_to(
                finite_f32(*control_1_x, "PNG fallback label control x")?,
                finite_f32(*control_1_y, "PNG fallback label control y")?,
                finite_f32(*control_2_x, "PNG fallback label control x")?,
                finite_f32(*control_2_y, "PNG fallback label control y")?,
                finite_f32(*x, "PNG fallback label x")?,
                finite_f32(*y, "PNG fallback label y")?,
            ),
            super::text::LabelPathOperation::Close => builder.close(),
        }
    }
    builder
        .finish()
        .ok_or_else(|| FormatEncodeError::Encoding("PNG fallback label has no outline".to_owned()))
}

#[allow(clippy::too_many_arguments)]
fn paint_rough_drawing(
    surface: &mut PixmapMut<'_>,
    drawing: &Drawing,
    transform: Transform,
    stroke_color: CanonicalColor,
    fill_color: Option<CanonicalColor>,
    stroke_width: f64,
    stroke_style: StrokeStyle,
    source_stroke_width: f64,
    opacity: f64,
) -> Result<(), FormatEncodeError> {
    if let (Some(fill), Some(fill_color)) = (&drawing.fill, fill_color) {
        let path = tiny_skia_path(&fill.path)?;
        let paint = solid_paint(fill_color, opacity)?;
        match fill.kind {
            FillKind::Solid => surface.fill_path(
                &path,
                &paint,
                if drawing.shape.canvas_uses_even_odd_fill() {
                    FillRule::EvenOdd
                } else {
                    FillRule::Winding
                },
                transform,
                None,
            ),
            FillKind::Sketch => {
                let stroke = Stroke {
                    width: finite_f32(source_stroke_width / 2.0, "PNG fill weight")?,
                    miter_limit: 4.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    dash: None,
                };
                surface.stroke_path(&path, &paint, &stroke, transform, None);
            }
        }
    }
    let path = tiny_skia_path(&drawing.outline)?;
    let paint = solid_paint(stroke_color, opacity)?;
    let stroke = Stroke {
        width: finite_f32(stroke_width, "PNG stroke width")?,
        miter_limit: 4.0,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        dash: match stroke_style {
            StrokeStyle::Solid => None,
            StrokeStyle::Dashed => StrokeDash::new(
                vec![8.0, finite_f32(8.0 + source_stroke_width, "PNG dash gap")?],
                0.0,
            ),
            StrokeStyle::Dotted => StrokeDash::new(
                vec![1.5, finite_f32(6.0 + source_stroke_width, "PNG dot gap")?],
                0.0,
            ),
        },
    };
    let resolution_scale = PathStroker::compute_resolution_scale(&transform);
    let path = path.stroke(&stroke, resolution_scale).ok_or_else(|| {
        FormatEncodeError::Encoding("PNG raster adapter could not stroke the Rough path".to_owned())
    })?;
    let path = path.transform(transform).ok_or_else(|| {
        FormatEncodeError::Encoding(
            "PNG raster adapter could not transform the stroked path".to_owned(),
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
    Ok(())
}

fn solid_paint(color: CanonicalColor, opacity: f64) -> Result<Paint<'static>, FormatEncodeError> {
    let rgb = color.rgb8();
    let color = Color::from_rgba(
        f32::from(rgb[0]) / 255.0,
        f32::from(rgb[1]) / 255.0,
        f32::from(rgb[2]) / 255.0,
        finite_f32(opacity * color.alpha(), "PNG paint opacity")?,
    )
    .ok_or_else(|| FormatEncodeError::Encoding("PNG paint is outside RGBA range".to_owned()))?;
    Ok(Paint {
        shader: Shader::SolidColor(color),
        blend_mode: BlendMode::SourceOver,
        anti_alias: true,
        force_hq_pipeline: true,
    })
}

fn conservative_coverage_area(
    drawing: &Drawing,
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
    for operation in drawing
        .fill
        .iter()
        .flat_map(|fill| &fill.path.operations)
        .chain(&drawing.outline.operations)
    {
        match operation {
            PathOperation::Move(point) => include(*point),
            PathOperation::Line(point) => include(*point),
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
    for decoration in &drawing.decorations {
        for operation in decoration
            .drawing
            .fill
            .iter()
            .flat_map(|fill| &fill.path.operations)
            .chain(&decoration.drawing.outline.operations)
        {
            match operation {
                PathOperation::Move(point) | PathOperation::Line(point) => include(*point),
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

fn conservative_fallback_coverage_area(
    width: f64,
    height: f64,
    transform: PngTransform,
    surface_width: u32,
    surface_height: u32,
) -> u64 {
    let (left, top, right, bottom) =
        fallback_coverage_bounds(width, height, transform, surface_width, surface_height);
    u64::from(right.saturating_sub(left)).saturating_mul(u64::from(bottom.saturating_sub(top)))
}

fn fallback_coverage_bounds(
    width: f64,
    height: f64,
    transform: PngTransform,
    surface_width: u32,
    surface_height: u32,
) -> (u32, u32, u32, u32) {
    let points = [
        Point { x: 0.0, y: 0.0 },
        Point { x: width, y: 0.0 },
        Point {
            x: width,
            y: height,
        },
        Point { x: 0.0, y: height },
    ]
    .map(|point| transform.map_point(point));
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let left = min_x.floor().clamp(0.0, f64::from(surface_width));
    let top = min_y.floor().clamp(0.0, f64::from(surface_height));
    let right = max_x.ceil().clamp(0.0, f64::from(surface_width));
    let bottom = max_y.ceil().clamp(0.0, f64::from(surface_height));
    (left as u32, top as u32, right as u32, bottom as u32)
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
