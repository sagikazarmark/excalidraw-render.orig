//! tiny-skia lowering for the PNG rasterizer: Rough drawings, freedraw
//! outlines, label glyph paths and the Font Awesome placeholder icons become
//! tiny-skia paths and paints here, the reference canvas's theme filter is
//! applied to rasterized nested-SVG layers, the supersampled surface is
//! box-filtered down, and `PaintMasks` keeps the shared frame-clip masks and
//! the per-element scratch mask one rasterization reuses.

use tiny_skia::{
    BlendMode, Color, ColorU8, FillRule, IntSize, LineCap, LineJoin, Mask, Paint, PathBuilder,
    PathStroker, Pixmap, PixmapMut, Rect, Shader, Stroke, StrokeDash, Transform,
};

use crate::color::{self, CanonicalColor};

use super::super::geometry::{Path, PathOperation};
use super::super::resolve::StrokeStyle;
use super::super::rough::{Drawing, FillKind};
use super::super::text::{LabelPath, LabelPathOperation};
use super::super::{freedraw, FormatEncodeError};
use super::coverage::frame_clip_coverage_bounds;
use super::encoder::png_size_error;
use super::{frame_clip_set_key, PngElement, PngFrameClip, PngLabelMask};

/// The canvas's unmasked N32 image blit: the already-resampled device pixel
/// fills one supersample block, using its byte source-over arithmetic. Keeping
/// the destination product at /256 (truncated) matters over nonwhite content.
pub(super) fn composite_unmasked_raster_blocks(
    surface: &mut PixmapMut<'_>,
    layer: &Pixmap,
    left: u32,
    top: u32,
    scale: u32,
) -> Result<(), FormatEncodeError> {
    let width = surface.width() as usize;
    let scale = scale as usize;
    let layer_width = layer.width() as usize;
    let data = surface.data_mut();
    for (y, row) in layer.data().chunks_exact(layer_width * 4).enumerate() {
        for sub_y in 0..scale {
            let start = (((top as usize + y) * scale + sub_y) * width + left as usize * scale) * 4;
            let target = data
                .get_mut(start..start + layer_width * scale * 4)
                .ok_or_else(|| {
                    FormatEncodeError::Encoding(
                        "PNG raster block bounds invariant failed".to_owned(),
                    )
                })?;
            for (source, block) in row.chunks_exact(4).zip(target.chunks_exact_mut(scale * 4)) {
                let inverse = 256 - u16::from(source[3]);
                for pixel in block.chunks_exact_mut(4) {
                    for (destination, source) in pixel.iter_mut().zip(source) {
                        *destination =
                            (u16::from(*source) + ((u16::from(*destination) * inverse) >> 8)) as u8;
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn downsample_rgba(
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

/// Lowers the reference canvas's theme filter into a rasterized layer: each
/// pixel's unpremultiplied color passes through the filter and is premultiplied
/// again, as the canvas filter reads and writes its surface; transparent pixels
/// have no color to filter.
pub(super) fn apply_theme_filter(layer: &mut Pixmap) {
    for pixel in layer.pixels_mut() {
        let alpha = pixel.alpha();
        if alpha == 0 {
            continue;
        }
        let color = pixel.demultiply();
        let [red, green, blue] = color::theme_filter([color.red(), color.green(), color.blue()]);
        *pixel = ColorU8::from_rgba(red, green, blue, alpha).premultiply();
    }
}

pub(super) fn tiny_skia_path(path: &Path) -> Result<tiny_skia::Path, FormatEncodeError> {
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

pub(super) fn tiny_skia_freedraw_path(
    path: &freedraw::ProjectedPath,
) -> Result<tiny_skia::Path, FormatEncodeError> {
    let points = path
        .operations
        .len()
        .checked_mul(2)
        .ok_or_else(png_size_error)?;
    let mut builder = PathBuilder::with_capacity(path.operations.len(), points);
    for operation in &path.operations {
        match operation {
            freedraw::PathOperation::Move(point) => builder.move_to(
                finite_f32(point.x, "PNG freedraw x")?,
                finite_f32(point.y, "PNG freedraw y")?,
            ),
            freedraw::PathOperation::Quadratic { control, end } => builder.quad_to(
                finite_f32(control.x, "PNG freedraw control x")?,
                finite_f32(control.y, "PNG freedraw control y")?,
                finite_f32(end.x, "PNG freedraw x")?,
                finite_f32(end.y, "PNG freedraw y")?,
            ),
            freedraw::PathOperation::Line(point) => builder.line_to(
                finite_f32(point.x, "PNG freedraw x")?,
                finite_f32(point.y, "PNG freedraw y")?,
            ),
            freedraw::PathOperation::Close => builder.close(),
        }
    }
    builder.finish().ok_or_else(|| {
        FormatEncodeError::Encoding("PNG freedraw path contains no outline".to_owned())
    })
}

pub(super) fn fallback_paths(
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

/// The raster path for one laid-out label line, or `None` when the line has no
/// outline to paint: an empty line, a whitespace-only line, or one whose glyphs
/// all lack outlines. Such a line still occupies its line step; it just paints
/// nothing, as in SVG and PDF (#111).
pub(super) fn label_path(path: &LabelPath) -> Result<Option<tiny_skia::Path>, FormatEncodeError> {
    if path.operations.is_empty() {
        return Ok(None);
    }
    let point_capacity = path
        .operations
        .len()
        .checked_mul(3)
        .ok_or_else(png_size_error)?;
    let mut builder = PathBuilder::with_capacity(path.operations.len(), point_capacity);
    for operation in &path.operations {
        match operation {
            LabelPathOperation::Move { x, y } => builder.move_to(
                finite_f32(*x, "PNG fallback label x")?,
                finite_f32(*y, "PNG fallback label y")?,
            ),
            LabelPathOperation::Line { x, y } => builder.line_to(
                finite_f32(*x, "PNG fallback label x")?,
                finite_f32(*y, "PNG fallback label y")?,
            ),
            LabelPathOperation::Quadratic {
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
            LabelPathOperation::Cubic {
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
            LabelPathOperation::Close => builder.close(),
        }
    }
    // `finish` declines a lone move, which has nothing to fill either.
    Ok(builder.finish())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_rough_drawing(
    surface: &mut PixmapMut<'_>,
    drawing: &Drawing,
    transform: Transform,
    stroke_color: CanonicalColor,
    fill_color: Option<CanonicalColor>,
    stroke_width: f64,
    stroke_style: StrokeStyle,
    source_stroke_width: f64,
    opacity: f64,
    mask: Option<&Mask>,
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
                mask,
            ),
            FillKind::Sketch => {
                let stroke = Stroke {
                    width: finite_f32(source_stroke_width / 2.0, "PNG fill weight")?,
                    miter_limit: 4.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    dash: None,
                };
                surface.stroke_path(&path, &paint, &stroke, transform, mask);
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
    // `Stroke::dash` is inert for the low-level stroker: tiny-skia applies it by
    // dashing the path first, exactly as its own painter does. Stroking without
    // this step silently drops dashed and dotted stroke styles.
    let dashed_path;
    let path = if let Some(dash) = &stroke.dash {
        dashed_path = path.dash(dash, resolution_scale).ok_or_else(|| {
            FormatEncodeError::Encoding(
                "PNG raster adapter could not dash the Rough path".to_owned(),
            )
        })?;
        dashed_path
    } else {
        path
    };
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
        mask,
    );
    Ok(())
}

/// The alpha masks one PNG rasterization reuses across elements.
///
/// Every element a frame clips paints through that frame's mask. tiny-skia
/// composites a mask only when it matches the surface, so one full raster
/// surface holds whichever frame clip mask the current element needs; each
/// distinct frame clip set is rasterized once, its covered rows are kept as a
/// stamp, and switching frames clears the previous stamp's rows and copies the
/// next one in, so the work of a switch is bounded by the two frame areas
/// rather than the canvas. An element that also excludes a bound label or clips
/// to its own image rectangle paints through one scratch mask derived from the
/// frame mask.
pub(super) struct PaintMasks {
    width: u32,
    height: u32,
    multiplier: u32,
    frame: Option<Mask>,
    stamps: Vec<FrameClipStamp>,
    current_stamp: Option<usize>,
    element: Option<Mask>,
}

/// The rows of one frame clip set's mask that its clip covers, at raster
/// resolution.
struct FrameClipStamp {
    key: Vec<usize>,
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl PaintMasks {
    pub(super) fn new(width: u32, height: u32, multiplier: u32) -> Self {
        Self {
            width,
            height,
            multiplier,
            frame: None,
            stamps: Vec::new(),
            current_stamp: None,
            element: None,
        }
    }

    /// The mask `element` paints through, if any: its frame clip mask, or a
    /// scratch mask that also carries its bound-label exclusion or image clip.
    pub(super) fn paint_mask(
        &mut self,
        element: &PngElement,
    ) -> Result<Option<&Mask>, FormatEncodeError> {
        let clips = element.frame_clips();
        let label_mask = element.label_mask();
        let image_clip =
            element.image_clip(self.width / self.multiplier, self.height / self.multiplier);
        if clips.is_empty() && label_mask.is_none() && image_clip.is_none() {
            return Ok(None);
        }
        self.ensure_frame_mask(clips)?;
        if label_mask.is_none() && image_clip.is_none() {
            return Ok(self.frame.as_ref());
        }
        let mut mask = match self.element.take() {
            Some(mask) => mask,
            None => self.blank_mask("png_paint_mask")?,
        };
        match &self.frame {
            Some(frame) if !clips.is_empty() => mask.data_mut().copy_from_slice(frame.data()),
            _ => mask.data_mut().fill(255),
        }
        if let Some(label) = label_mask {
            self.exclude_label(&mut mask, label)?;
        }
        if let Some(clip) = image_clip {
            let raster_transform = clip.transform.raster_transform(self.multiplier)?;
            match clip.corner_clip {
                Some(path) => mask.intersect_path(
                    &tiny_skia_path(path)?,
                    FillRule::Winding,
                    true,
                    raster_transform,
                ),
                None => {
                    let rect = Rect::from_xywh(
                        0.0,
                        0.0,
                        finite_f32(clip.width, "PNG image clip width")?,
                        finite_f32(clip.height, "PNG image clip height")?,
                    )
                    .ok_or_else(png_size_error)?;
                    mask.intersect_path(
                        &PathBuilder::from_rect(rect),
                        FillRule::Winding,
                        true,
                        raster_transform,
                    );
                }
            }
        }
        Ok(Some(self.element.insert(mask)))
    }

    /// Makes `self.frame` the mask of `clips`: the stamp of a frame clip set
    /// seen before is copied back in, a new set is rasterized once and stamped.
    fn ensure_frame_mask(&mut self, clips: &[PngFrameClip]) -> Result<(), FormatEncodeError> {
        if clips.is_empty() {
            return Ok(());
        }
        let wanted = self
            .stamps
            .iter()
            .position(|stamp| stamp.key.iter().copied().eq(frame_clip_set_key(clips)));
        if self.frame.is_some() && wanted.is_some() && wanted == self.current_stamp {
            return Ok(());
        }
        if self.frame.is_none() {
            self.frame = Some(self.blank_mask("png_frame_clip")?);
        }
        if let Some(previous) = self.current_stamp.take() {
            self.clear_stamp_rows(previous);
        }
        let index = match wanted {
            Some(index) => {
                self.copy_stamp_in(index);
                index
            }
            None => self.rasterize_stamp(clips)?,
        };
        self.current_stamp = Some(index);
        Ok(())
    }

    /// Rasterizes `clips` into the (cleared) frame mask and records the rows
    /// its coverage bounds span as a new stamp.
    fn rasterize_stamp(&mut self, clips: &[PngFrameClip]) -> Result<usize, FormatEncodeError> {
        // Coverage bounds are conservative in canvas pixels; the stamp scales
        // them to the raster and pads one raster pixel each side so that f32
        // edge rounding in the rasterizer can never leave coverage behind.
        let multiplier = usize::try_from(self.multiplier).map_err(|_| png_size_error())?;
        let raster_width = usize::try_from(self.width).map_err(|_| png_size_error())?;
        let raster_height = usize::try_from(self.height).map_err(|_| png_size_error())?;
        let (left, top, width, height) = frame_clip_coverage_bounds(
            clips,
            self.width / self.multiplier,
            self.height / self.multiplier,
        )
        .map_or((0, 0, 0, 0), |(left, top, right, bottom)| {
            let left = (left as usize * multiplier).saturating_sub(1);
            let top = (top as usize * multiplier).saturating_sub(1);
            let right = (right as usize * multiplier + 1).min(raster_width);
            let bottom = (bottom as usize * multiplier + 1).min(raster_height);
            (left, top, right - left, bottom - top)
        });
        let mask = self.frame.as_mut().ok_or_else(png_size_error)?;
        for (index, clip) in clips.iter().enumerate() {
            let path = tiny_skia_path(&clip.path)?;
            let transform = clip.transform.raster_transform(self.multiplier)?;
            if index == 0 {
                mask.fill_path(&path, FillRule::Winding, true, transform);
            } else {
                mask.intersect_path(&path, FillRule::Winding, true, transform);
            }
        }
        let memory = |_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "raster_surface",
            subject: "png_frame_clip_stamp",
        };
        let mut key = Vec::new();
        key.try_reserve_exact(clips.len()).map_err(memory)?;
        key.extend(frame_clip_set_key(clips));
        let mut data = Vec::new();
        data.try_reserve_exact(width.checked_mul(height).ok_or_else(png_size_error)?)
            .map_err(memory)?;
        for row in top..top + height {
            let start = row * raster_width + left;
            data.extend_from_slice(&mask.data()[start..start + width]);
        }
        self.stamps.try_reserve(1).map_err(memory)?;
        self.stamps.push(FrameClipStamp {
            key,
            left,
            top,
            width,
            height,
            data,
        });
        Ok(self.stamps.len() - 1)
    }

    fn clear_stamp_rows(&mut self, index: usize) {
        let Some((mask, stamp)) = self.frame.as_mut().zip(self.stamps.get(index)) else {
            return;
        };
        let stride = self.width as usize;
        let data = mask.data_mut();
        for row in stamp.top..stamp.top + stamp.height {
            let start = row * stride + stamp.left;
            data[start..start + stamp.width].fill(0);
        }
    }

    fn copy_stamp_in(&mut self, index: usize) {
        let Some((mask, stamp)) = self.frame.as_mut().zip(self.stamps.get(index)) else {
            return;
        };
        let stride = self.width as usize;
        let data = mask.data_mut();
        for (offset, row) in (stamp.top..stamp.top + stamp.height).enumerate() {
            let start = row * stride + stamp.left;
            let source = offset * stamp.width;
            data[start..start + stamp.width]
                .copy_from_slice(&stamp.data[source..source + stamp.width]);
        }
    }

    fn blank_mask(&self, subject: &'static str) -> Result<Mask, FormatEncodeError> {
        let mask_bytes = usize::try_from(self.width)
            .ok()
            .and_then(|width| width.checked_mul(usize::try_from(self.height).ok()?))
            .ok_or_else(png_size_error)?;
        let mut data = Vec::new();
        data.try_reserve_exact(mask_bytes)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "raster_surface",
                subject,
            })?;
        data.resize(mask_bytes, 0);
        let size = IntSize::from_wh(self.width, self.height).ok_or_else(png_size_error)?;
        Mask::from_vec(data, size).ok_or_else(|| {
            FormatEncodeError::Encoding("PNG paint mask dimensions are invalid".to_owned())
        })
    }

    fn exclude_label(&self, mask: &mut Mask, label: PngLabelMask) -> Result<(), FormatEncodeError> {
        let multiplier = f64::from(self.multiplier);
        let left = (label.x * multiplier).floor().max(0.0) as u32;
        let top = (label.y * multiplier).floor().max(0.0) as u32;
        let right = ((label.x + label.width) * multiplier)
            .ceil()
            .clamp(0.0, f64::from(self.width)) as u32;
        let bottom = ((label.y + label.height) * multiplier)
            .ceil()
            .clamp(0.0, f64::from(self.height)) as u32;
        let data = mask.data_mut();
        for y in top.min(self.height)..bottom {
            let row = usize::try_from(y)
                .ok()
                .and_then(|value| value.checked_mul(usize::try_from(self.width).ok()?))
                .ok_or_else(png_size_error)?;
            for x in left.min(self.width)..right {
                let index = row
                    .checked_add(usize::try_from(x).map_err(|_| png_size_error())?)
                    .ok_or_else(png_size_error)?;
                data[index] = 0;
            }
        }
        Ok(())
    }
}

pub(super) const OFFICIAL_IMAGE_PATH: &str = "M464 448H48c-26.51 0-48-21.49-48-48V112c0-26.51 21.49-48 48-48h416c26.51 0 48 21.49 48 48v288c0 26.51-21.49 48-48 48zM112 120c-30.928 0-56 25.072-56 56s25.072 56 56 56 56-25.072 56-56-25.072-56-56-56zM64 384h384V272l-87.515-87.515c-4.686-4.686-12.284-4.686-16.971 0L208 320l-55.515-55.515c-4.686-4.686-12.284-4.686-16.971 0L64 336v48z";
pub(super) const OFFICIAL_IMAGE_ERROR_PATH: &str = "M464 448H48c-26.51 0-48-21.49-48-48V112c0-26.51 21.49-48 48-48h416c26.51 0 48 21.49 48 48v288c0 26.51-21.49 48-48 48ZM112 120c-30.928 0-56 25.072-56 56s25.072 56 56 56 56-25.072 56-56-25.072-56-56-56ZM64 384h384V272l-87.515-87.515c-4.686-4.686-12.284-4.686-16.971 0L208 320l-55.515-55.515c-4.686-4.686-12.284-4.686-16.971 0L64 336v48Z";
pub(super) const OFFICIAL_IMAGE_ERROR_BAN_PATH: &str = "M256 8C119.034 8 8 119.033 8 256c0 136.967 111.034 248 248 248s248-111.034 248-248S392.967 8 256 8Zm130.108 117.892c65.448 65.448 70 165.481 20.677 235.637L150.47 105.216c70.204-49.356 170.226-44.735 235.638 20.676ZM125.892 386.108c-65.448-65.448-70-165.481-20.677-235.637L361.53 406.784c-70.203 49.356-170.226 44.736-235.638-20.676Z";

pub(super) fn svg_path(data: &str) -> Result<tiny_skia::Path, FormatEncodeError> {
    use svgtypes::PathSegment;

    let mut builder = PathBuilder::new();
    let mut current = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    let mut cubic_control = None;
    let mut quadratic_control = None;
    let point = |abs: bool, x: f64, y: f64, current: (f64, f64)| {
        if abs {
            (x, y)
        } else {
            (current.0 + x, current.1 + y)
        }
    };
    for segment in svgtypes::PathParser::from(data) {
        let segment = segment.map_err(|_| {
            FormatEncodeError::Encoding("official image fallback path is invalid".to_owned())
        })?;
        match segment {
            PathSegment::MoveTo { abs, x, y } => {
                current = point(abs, x, y, current);
                start = current;
                builder.move_to(
                    finite_f32(current.0, "PNG fallback move x")?,
                    finite_f32(current.1, "PNG fallback move y")?,
                );
                cubic_control = None;
                quadratic_control = None;
            }
            PathSegment::LineTo { abs, x, y } => {
                current = point(abs, x, y, current);
                builder.line_to(
                    finite_f32(current.0, "PNG fallback line x")?,
                    finite_f32(current.1, "PNG fallback line y")?,
                );
                cubic_control = None;
                quadratic_control = None;
            }
            PathSegment::HorizontalLineTo { abs, x } => {
                current.0 = if abs { x } else { current.0 + x };
                builder.line_to(
                    finite_f32(current.0, "PNG fallback horizontal x")?,
                    finite_f32(current.1, "PNG fallback horizontal y")?,
                );
                cubic_control = None;
                quadratic_control = None;
            }
            PathSegment::VerticalLineTo { abs, y } => {
                current.1 = if abs { y } else { current.1 + y };
                builder.line_to(
                    finite_f32(current.0, "PNG fallback vertical x")?,
                    finite_f32(current.1, "PNG fallback vertical y")?,
                );
                cubic_control = None;
                quadratic_control = None;
            }
            PathSegment::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let control_1 = point(abs, x1, y1, current);
                let control_2 = point(abs, x2, y2, current);
                let end = point(abs, x, y, current);
                builder.cubic_to(
                    finite_f32(control_1.0, "PNG fallback cubic control x")?,
                    finite_f32(control_1.1, "PNG fallback cubic control y")?,
                    finite_f32(control_2.0, "PNG fallback cubic control x")?,
                    finite_f32(control_2.1, "PNG fallback cubic control y")?,
                    finite_f32(end.0, "PNG fallback cubic x")?,
                    finite_f32(end.1, "PNG fallback cubic y")?,
                );
                current = end;
                cubic_control = Some(control_2);
                quadratic_control = None;
            }
            PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                let control_1 = cubic_control.map_or(current, |control: (f64, f64)| {
                    (2.0 * current.0 - control.0, 2.0 * current.1 - control.1)
                });
                let control_2 = point(abs, x2, y2, current);
                let end = point(abs, x, y, current);
                builder.cubic_to(
                    finite_f32(control_1.0, "PNG fallback smooth control x")?,
                    finite_f32(control_1.1, "PNG fallback smooth control y")?,
                    finite_f32(control_2.0, "PNG fallback smooth control x")?,
                    finite_f32(control_2.1, "PNG fallback smooth control y")?,
                    finite_f32(end.0, "PNG fallback smooth x")?,
                    finite_f32(end.1, "PNG fallback smooth y")?,
                );
                current = end;
                cubic_control = Some(control_2);
                quadratic_control = None;
            }
            PathSegment::Quadratic { abs, x1, y1, x, y } => {
                let control = point(abs, x1, y1, current);
                let end = point(abs, x, y, current);
                builder.quad_to(
                    finite_f32(control.0, "PNG fallback quadratic control x")?,
                    finite_f32(control.1, "PNG fallback quadratic control y")?,
                    finite_f32(end.0, "PNG fallback quadratic x")?,
                    finite_f32(end.1, "PNG fallback quadratic y")?,
                );
                current = end;
                quadratic_control = Some(control);
                cubic_control = None;
            }
            PathSegment::SmoothQuadratic { abs, x, y } => {
                let control = quadratic_control.map_or(current, |control: (f64, f64)| {
                    (2.0 * current.0 - control.0, 2.0 * current.1 - control.1)
                });
                let end = point(abs, x, y, current);
                builder.quad_to(
                    finite_f32(control.0, "PNG fallback smooth quadratic control x")?,
                    finite_f32(control.1, "PNG fallback smooth quadratic control y")?,
                    finite_f32(end.0, "PNG fallback smooth quadratic x")?,
                    finite_f32(end.1, "PNG fallback smooth quadratic y")?,
                );
                current = end;
                quadratic_control = Some(control);
                cubic_control = None;
            }
            PathSegment::ClosePath { .. } => {
                builder.close();
                current = start;
                cubic_control = None;
                quadratic_control = None;
            }
            PathSegment::EllipticalArc { .. } => {
                return Err(FormatEncodeError::Encoding(
                    "official image fallback unexpectedly contains an arc".to_owned(),
                ));
            }
        }
    }
    builder.finish().ok_or_else(|| {
        FormatEncodeError::Encoding("official image fallback path is empty".to_owned())
    })
}

pub(super) fn solid_paint(
    color: CanonicalColor,
    opacity: f64,
) -> Result<Paint<'static>, FormatEncodeError> {
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

pub(super) fn finite_f32(value: f64, subject: &str) -> Result<f32, FormatEncodeError> {
    let narrowed = value as f32;
    if !value.is_finite() || !narrowed.is_finite() {
        return Err(FormatEncodeError::Encoding(format!(
            "{subject} must be finite in the PNG raster domain"
        )));
    }
    Ok(narrowed)
}

#[cfg(test)]
mod tests {
    use crate::render::text::{LabelPath, LabelPathOperation};

    use super::label_path;

    #[test]
    fn a_line_with_no_outline_has_no_path_to_fill() {
        // Every label call site shares this helper, so an empty line, a
        // whitespace-only line, or a lone move paints nothing rather than
        // failing the artifact (#111).
        let empty = LabelPath {
            operations: Vec::new(),
        };
        assert!(matches!(label_path(&empty), Ok(None)));

        let lone_move = LabelPath {
            operations: vec![LabelPathOperation::Move { x: 1.0, y: 2.0 }],
        };
        assert!(matches!(label_path(&lone_move), Ok(None)));

        let outline = LabelPath {
            operations: vec![
                LabelPathOperation::Move { x: 0.0, y: 0.0 },
                LabelPathOperation::Line { x: 4.0, y: 0.0 },
                LabelPathOperation::Line { x: 4.0, y: 4.0 },
                LabelPathOperation::Close,
            ],
        };
        let Ok(Some(path)) = label_path(&outline) else {
            panic!("an outline has a path");
        };
        assert_eq!(path.len(), 4);
    }
}
