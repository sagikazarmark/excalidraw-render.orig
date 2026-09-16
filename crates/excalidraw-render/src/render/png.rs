//! The PNG backend (stage 5 and 6): `PngPlan` lowers the resolved scene into
//! paint-ordered `PngElement`s in device space, `rasterize` paints them with
//! tiny-skia at a fixed 4x supersample and box-filters the result down, and
//! `compress` hands the straight-alpha raster to the owned encoder. The
//! per-index area queries and `omit_element` exist so the session can charge
//! the §13.2 raster ledger from the plan and drop what it cannot afford before
//! painting.
//!
//! `encoder` is the owned PNG encoder ADR-0004 describes — scanline filtering,
//! deflate, chunk assembly and the size-error vocabulary — with no dependency
//! on the plan or the painter; `paint` lowers
//! Rough drawings, freedraw outlines, labels and the Font Awesome placeholder
//! into tiny-skia paths and owns the shared frame-clip masks; `coverage` holds
//! the conservative device-space area estimators the ledger reads, which are
//! a pure function of the plan and never of the painted pixels.

mod coverage;
mod encoder;
mod paint;

pub(super) use encoder::PngIdatStream;
pub(super) use encoder::{canonicalize_straight_alpha, compress_rgba};

use coverage::{
    conservative_coverage_area, conservative_fallback_coverage_area,
    conservative_freedraw_coverage_area, conservative_text_coverage_area, fallback_coverage_bounds,
    frame_clip_coverage_area, frame_clip_coverage_bounds, label_mask_coverage_area,
};
use encoder::png_size_error;
use paint::{
    apply_theme_filter, downsample_rgba, fallback_paths, finite_f32, label_path,
    paint_rough_drawing, solid_paint, svg_path, tiny_skia_freedraw_path, tiny_skia_path,
    PaintMasks, OFFICIAL_IMAGE_ERROR_BAN_PATH, OFFICIAL_IMAGE_ERROR_PATH, OFFICIAL_IMAGE_PATH,
};

use std::sync::Arc;

use crate::normalization::{NormalizedElementSource, NormalizedImageStatus};
use crate::portable_math::sin_cos;
use crate::{color::CanonicalColor, OutputFormat, RenderRequest};
use tiny_skia::{
    Color, FillRule, FilterQuality, IntSize, LineCap, LineJoin, Mask, PathBuilder, Pixmap,
    PixmapMut, PixmapPaint, PixmapRef, Rect, Stroke, StrokeDash, Transform,
};

use super::geometry::{Path, Point, FRAME_OUTLINE_STROKE_WIDTH};
use super::notices::{MandatoryNotices, NoticeError};
use super::plan::PlannedElement;
use super::resolve::{
    decoration_fill_color, decoration_stroke_style, DarkModeImageFilter, FallbackColors,
    FallbackSource, ResolvedClip, ResolvedElement, ResolvedFrameOutline, ResolvedImageAsset,
    ResolvedImageSourceRect, ResolvedScene, ResolvedTransform, StrokeStyle,
};
use super::rough::{Drawing, Fill, FillKind};
use super::text::{ResolvedLabel, ResolvedText};
use super::{freedraw, FormatEncodeError, SvgTreeCache};

#[derive(Debug)]
pub(super) struct PngPlan {
    width: u32,
    height: u32,
    background: Option<[u8; 4]>,
    fallback_colors: FallbackColors,
    /// `drawImagePlaceholder`'s `#E7E7E7` rectangle, for the color mode.
    image_fallback_background: CanonicalColor,
    /// `drawImagePlaceholder`'s `#888` Font Awesome glyph, for the color mode.
    image_fallback_icon_color: CanonicalColor,
    svg_trees: SvgTreeCache,
    elements: Vec<PngElement>,
}

/// `drawImagePlaceholder`: the reference canvas fills the missing image's
/// rectangle `#E7E7E7` and draws the `#888` Font Awesome glyph over it. A
/// missing image is pending in the reference's image cache, so the canvas
/// keeps its theme filter for the placeholder and both colors pass through it.
const IMAGE_FALLBACK_BACKGROUND: CanonicalColor = CanonicalColor::from_rgb8(0xe7, 0xe7, 0xe7);
const IMAGE_FALLBACK_ICON_COLOR: CanonicalColor = CanonicalColor::from_rgb8(0x88, 0x88, 0x88);

/// One PNG plan element: the shared header in device space, the kind, and
/// whether the session still paints it.
#[derive(Debug)]
struct PngElement {
    planned: PlannedElement<PngTransform, PngFrameClip, PngElementKind>,
    /// Cleared by `omit_element` for an element the session cannot afford
    /// after planning. An omitted element keeps its plan, so the shared
    /// frame-clip masks and the per-index area queries still see it.
    render: bool,
}

/// What differs per element kind in the PNG plan; everything else an element
/// carries is the [`PlannedElement`] header. The PNG plan never writes the
/// header's link.
#[derive(Debug)]
enum PngElementKind {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        background_color: CanonicalColor,
        label_mask: Option<PngLabelMask>,
        generated_label: Option<ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
        base_element_work_units: u64,
    },
    Freedraw {
        source: NormalizedElementSource,
        path: freedraw::ProjectedPath,
        background_fill: Option<Fill>,
        color: CanonicalColor,
        background_color: Option<CanonicalColor>,
        source_stroke_width: f64,
        base_element_work_units: u64,
    },
    Frame {
        source: NormalizedElementSource,
        name: Option<PngFrameName>,
        outline: Option<ResolvedFrameOutline>,
    },
    Text {
        source: NormalizedElementSource,
        text: ResolvedText,
        color: CanonicalColor,
        base_element_work_units: u64,
    },
    Image {
        source: NormalizedElementSource,
        asset: Arc<ResolvedImageAsset>,
        /// The dark-mode filter the canvas applies to this image. The canvas
        /// resets its filter for a raster image, so only a nested-SVG image's
        /// rasterized pixels are lowered here.
        dark_mode_filter: Option<DarkModeImageFilter>,
        /// The rounded-corner clip of an image with a roundness record, in
        /// element space; without one the image clips to its rectangle.
        corner_clip: Option<Path>,
        source_rect: ResolvedImageSourceRect,
        scale: [i8; 2],
    },
    ImageFallback {
        source: NormalizedElementSource,
        status: NormalizedImageStatus,
    },
    Fallback {
        source: FallbackSource,
        label: Option<ResolvedLabel>,
        base_element_work_units: u64,
    },
}

#[derive(Debug)]
struct PngFrameClip {
    frame_source_index: usize,
    path: Path,
    transform: PngTransform,
}

/// A frame's name at its own unrotated placement above the frame, painted at
/// full opacity and outside the frame's clip like the text element the
/// reference synthesizes for it.
#[derive(Debug)]
struct PngFrameName {
    label: ResolvedLabel,
    transform: PngTransform,
    color: CanonicalColor,
}

/// One frame clip mask the PNG plan builds once and shares across every
/// element that frame clips.
#[derive(Clone, Copy, Debug)]
pub(super) struct PngFrameClipMask {
    /// Plan index of the clipping frame's own element, when it is emitted.
    pub(super) frame_element_index: Option<usize>,
    /// Clipped conservative device-space area of the frame clip in canvas
    /// pixels: the alpha-mask generation cost `P` of the shared mask.
    pub(super) pixels: u64,
}

#[derive(Clone, Copy, Debug)]
struct PngLabelMask {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn project_frame_clips(
    clips: Vec<ResolvedClip>,
    scale: f64,
) -> Result<Vec<PngFrameClip>, FormatEncodeError> {
    let mut projected = Vec::new();
    projected
        .try_reserve_exact(clips.len())
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_plan",
            subject: "png_frame_clips",
        })?;
    for clip in clips {
        projected.push(PngFrameClip {
            frame_source_index: clip.frame_source_index,
            path: clip.path,
            transform: PngTransform::new(&clip.transform, scale),
        });
    }
    Ok(projected)
}

/// The identity of a frame clip set: the clipping frames, in order.
fn frame_clip_set_key(clips: &[PngFrameClip]) -> impl Iterator<Item = usize> + '_ {
    clips.iter().map(|clip| clip.frame_source_index)
}

/// Whether two clip sets are the same frame clip set: the same frames, in the
/// same order.
fn same_frame_clip_set(left: &[PngFrameClip], right: &[PngFrameClip]) -> bool {
    frame_clip_set_key(left).eq(frame_clip_set_key(right))
}

impl PngElement {
    /// A freshly planned element, painted unless the session omits it.
    const fn planned(planned: PlannedElement<PngTransform, PngFrameClip, PngElementKind>) -> Self {
        Self {
            planned,
            render: true,
        }
    }

    fn painted_font_awesome_status(
        &self,
        surface_width: u32,
        surface_height: u32,
    ) -> Option<NormalizedImageStatus> {
        let PlannedElement {
            frame_clips,
            transform,
            opacity,
            width,
            height,
            kind: PngElementKind::ImageFallback { status, .. },
            ..
        } = &self.planned
        else {
            return None;
        };
        if !(self.render && *width > 0.0 && *height > 0.0 && *opacity > 0.0) {
            return None;
        }
        let (left, top, right, bottom) =
            fallback_coverage_bounds(*width, *height, *transform, surface_width, surface_height);
        let (clip_left, clip_top, clip_right, clip_bottom) =
            frame_clip_coverage_bounds(frame_clips, surface_width, surface_height)?;
        let left = left.max(clip_left);
        let top = top.max(clip_top);
        let right = right.min(clip_right);
        let bottom = bottom.min(clip_bottom);
        (left < right && top < bottom).then_some(*status)
    }

    /// The frame clips the element paints through the shared frame-clip mask.
    /// A compatibility fallback paints in its own layer and intersects its
    /// frame clips into that layer's clip instead, so it shares no mask.
    fn frame_clips(&self) -> &[PngFrameClip] {
        if matches!(self.planned.kind, PngElementKind::Fallback { .. }) {
            &[]
        } else {
            &self.planned.frame_clips
        }
    }

    fn label_mask(&self) -> Option<PngLabelMask> {
        match &self.planned.kind {
            PngElementKind::RoughOutline { label_mask, .. } => *label_mask,
            _ => None,
        }
    }

    /// The element-space clip an image or its placeholder paints through: the
    /// element rectangle, or an image's rounded-corner outline when it has a
    /// roundness record. The placeholder is never corner-clipped: the
    /// reference's `drawElementOnCanvas` image case calls `roundRect` and
    /// `clip` only before `drawImage`, not around `drawImagePlaceholder`. An
    /// axis-aligned image whose destination rectangle is snapped to pixel
    /// centres ([`PngElement::snapped_image_rect`]) needs no rectangle clip:
    /// the snapped rectangle is what it paints, and a rectangle mask at the
    /// fractional edge would cut the row the canvas paints whole (#162).
    fn image_clip(&self, raster_width: u32, raster_height: u32) -> Option<PngImageClip<'_>> {
        let corner_clip = match &self.planned.kind {
            PngElementKind::Image { corner_clip, .. } => corner_clip.as_ref(),
            PngElementKind::ImageFallback { .. } => None,
            _ => return None,
        };
        if corner_clip.is_none()
            && self
                .snapped_image_rect(raster_width, raster_height)
                .is_some()
        {
            return None;
        }
        Some(PngImageClip {
            width: self.planned.width,
            height: self.planned.height,
            corner_clip,
            transform: self.planned.transform,
        })
    }

    /// The device-pixel rectangle an axis-aligned raster image paints, or
    /// `None` when the image is a nested SVG, is rotated, is thinner than a
    /// device pixel, or lies entirely off the `raster_width` by
    /// `raster_height` canvas, in which case it paints with antialiased edges
    /// like any other shape.
    ///
    /// The reference canvas draws a raster image whose current transform
    /// keeps rectangles rectangles (rotation 0, any flip, any scale) without
    /// edge antialiasing, as Chromium's `drawImage` does for a destination at
    /// least one device pixel wide and high: Skia rounds each edge of the
    /// destination rectangle to `floor(edge + 0.5)`, so a pixel is painted if
    /// and only if its centre lies inside the rectangle, a centre on the left
    /// or top edge falling outside and one on the right or bottom edge inside.
    /// A fractional edge is therefore a whole row or column or none, never a
    /// blend, while the image is still sampled bilinearly at the exact
    /// fractional mapping (#162). The rectangle is in canvas pixels before the
    /// supersample; the painter scales it by the fixed multiplier, where its
    /// edges land on whole subpixels and the box filter leaves them whole. A
    /// nested-SVG image is not snapped: the canvas draws it as vectors whose
    /// own edge is antialiased and clips it to the destination with a second
    /// antialiased edge (see `clip_nested_svg_like_the_canvas`).
    fn snapped_image_rect(&self, raster_width: u32, raster_height: u32) -> Option<SnappedRect> {
        let PngElementKind::Image { asset, .. } = &self.planned.kind else {
            return None;
        };
        if asset.sanitized_svg.is_some() {
            return None;
        }
        let (left, top, right, bottom) = self.axis_aligned_device_rect()?;
        if !(right - left >= 1.0 && bottom - top >= 1.0) {
            return None;
        }
        let snap = |edge: f64, limit: u32| (edge + 0.5).floor().clamp(0.0, f64::from(limit)) as u32;
        let rect = SnappedRect {
            left: snap(left, raster_width),
            top: snap(top, raster_height),
            right: snap(right, raster_width),
            bottom: snap(bottom, raster_height),
        };
        (rect.left < rect.right && rect.top < rect.bottom).then_some(rect)
    }

    /// The element rectangle in canvas pixels, `(left, top, right, bottom)`,
    /// when the element's transform keeps rectangles rectangles (rotation 0,
    /// any flip, any scale); `None` for a rotated element.
    fn axis_aligned_device_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let transform = self.planned.transform;
        if transform.b != 0.0 || transform.c != 0.0 {
            return None;
        }
        let origin = transform.map_point(Point { x: 0.0, y: 0.0 });
        let corner = transform.map_point(Point {
            x: self.planned.width,
            y: self.planned.height,
        });
        Some((
            origin.x.min(corner.x),
            origin.y.min(corner.y),
            origin.x.max(corner.x),
            origin.y.max(corner.y),
        ))
    }
}

/// Applies to a nested-SVG image's supersampled layer the two clips the
/// reference canvas applies when it draws an SVG image through `drawImage`
/// with an axis-aligned transform (Blink `SVGImage::DrawInternal`, #162):
///
/// - the SVG document's own viewport clip, antialiased, at the fractional
///   destination rectangle `rect` (canvas pixels): the layer is multiplied by
///   the destination's area coverage of each canvas pixel, so a fractional edge
///   row or column, whose content resvg already antialiases to its coverage,
///   carries the square of that coverage as the canvas does;
/// - `clipRect(ToEnclosingRect(dst_rect))`, a non-antialiased clip of the
///   destination rectangle enlarged to whole element-space pixels, `enclosing`
///   in canvas pixels, which Skia snaps to device pixels by rounding each edge
///   to `floor(edge + 0.5)`: a pixel whose centre lies at or left of the left
///   edge, or right of the right edge, is not painted at all.
///
/// `layer` is the supersampled layer whose top-left subpixel is `(layer_left,
/// layer_top)` on the surface and `multiplier` the supersample; every subpixel
/// of one canvas pixel takes that pixel's factor, so the box filter leaves the
/// product exact.
fn clip_nested_svg_like_the_canvas(
    layer: &mut Pixmap,
    layer_left: u32,
    layer_top: u32,
    rect: (f64, f64, f64, f64),
    enclosing: (f64, f64, f64, f64),
    multiplier: u32,
) {
    let (left, top, right, bottom) = rect;
    let coverage = |start: f64, end: f64, pixel: u32| -> f64 {
        let pixel = f64::from(pixel);
        (end.min(pixel + 1.0) - start.max(pixel)).clamp(0.0, 1.0)
    };
    let snapped = |edge: f64| (edge + 0.5).floor();
    let inside = |edge_start: f64, edge_end: f64, pixel: u32| -> bool {
        let pixel = f64::from(pixel);
        snapped(edge_start) <= pixel && pixel < snapped(edge_end)
    };
    let width = layer.width() as usize;
    let (layer_left, layer_top, multiplier) =
        (layer_left as usize, layer_top as usize, multiplier as usize);
    for (index, pixel) in layer.pixels_mut().iter_mut().enumerate() {
        let column = ((layer_left + index % width) / multiplier) as u32;
        let row = ((layer_top + index / width) / multiplier) as u32;
        let factor =
            if inside(enclosing.0, enclosing.2, column) && inside(enclosing.1, enclosing.3, row) {
                coverage(left, right, column) * coverage(top, bottom, row)
            } else {
                0.0
            };
        if factor >= 1.0 {
            continue;
        }
        let scale = |channel: u8| (f64::from(channel) * factor).round().clamp(0.0, 255.0) as u8;
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
            scale(pixel.red()),
            scale(pixel.green()),
            scale(pixel.blue()),
            scale(pixel.alpha()),
        )
        .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    }
}

/// The whole device pixels an axis-aligned image paints, in canvas pixels,
/// `left..right` by `top..bottom` (see [`PngElement::snapped_image_rect`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SnappedRect {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

/// The clip `PaintMasks::paint_mask` intersects into an image's mask, in the
/// element's own space under `transform`: the rounded-corner outline when the
/// image has one, else the `width` by `height` rectangle.
struct PngImageClip<'a> {
    width: f64,
    height: f64,
    corner_clip: Option<&'a Path>,
    transform: PngTransform,
}

#[derive(Clone, Copy, Debug)]
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
    fn new(transform: &ResolvedTransform, scale: f64) -> Self {
        let (sin, cos) = sin_cos(transform.angle_radians);
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

    fn with_local_offset(mut self, x: f64, y: f64) -> Self {
        self.e += self.a * x + self.c * y;
        self.f += self.b * x + self.d * y;
        self
    }

    /// The raster transform that fills one text line's glyph outlines, with
    /// the line's baseline snapped to the device pixel grid the way the
    /// reference canvas's text rasterization snaps it (spec 8.5, #169): Skia
    /// positions an axis-aligned glyph run's origin with horizontal subpixel
    /// precision and rounds its device y to `floor(y + 0.5)`, so every line the
    /// canvas draws with `fillText` sits on a whole device pixel while its x
    /// stays fractional. `baseline` is the line's baseline in the element's own
    /// space; the snap is taken after the export scale, in device pixels, and
    /// shifts the whole line's outlines by the same amount. A rotated line is
    /// not axis-aligned and keeps the exact baseline, as Skia positions a
    /// rotated run with subpixel precision on both axes.
    fn text_line_transform(
        self,
        baseline: f64,
        multiplier: u32,
    ) -> Result<Transform, FormatEncodeError> {
        if self.b != 0.0 || self.c != 0.0 {
            return self.raster_transform(multiplier);
        }
        let device_baseline = self.d * baseline + self.f;
        let snapped = (device_baseline + 0.5).floor();
        let mut snapped_transform = self;
        snapped_transform.f += snapped - device_baseline;
        snapped_transform.raster_transform(multiplier)
    }

    fn raster_image_transform(
        self,
        multiplier: u32,
        source: ResolvedImageSourceRect,
        width: f64,
        height: f64,
        scale: [i8; 2],
    ) -> Result<Transform, FormatEncodeError> {
        self.raster_image_transform_with_origin(multiplier, source, width, height, scale, 0, 0)
    }

    #[allow(clippy::too_many_arguments)]
    fn raster_image_transform_with_origin(
        self,
        multiplier: u32,
        source: ResolvedImageSourceRect,
        width: f64,
        height: f64,
        scale: [i8; 2],
        origin_x: u32,
        origin_y: u32,
    ) -> Result<Transform, FormatEncodeError> {
        let base_scale_x = width / source.width;
        let base_scale_y = height / source.height;
        let (scale_x, offset_x) = if scale[0] < 0 {
            (-base_scale_x, width + source.x * base_scale_x)
        } else {
            (base_scale_x, -source.x * base_scale_x)
        };
        let (scale_y, offset_y) = if scale[1] < 0 {
            (-base_scale_y, height + source.y * base_scale_y)
        } else {
            (base_scale_y, -source.y * base_scale_y)
        };
        let multiplier = f64::from(multiplier);
        let transform = Transform::from_row(
            finite_f32(self.a * scale_x * multiplier, "PNG image transform a")?,
            finite_f32(self.b * scale_x * multiplier, "PNG image transform b")?,
            finite_f32(self.c * scale_y * multiplier, "PNG image transform c")?,
            finite_f32(self.d * scale_y * multiplier, "PNG image transform d")?,
            finite_f32(
                (self.a * offset_x + self.c * offset_y + self.e) * multiplier - f64::from(origin_x),
                "PNG image transform e",
            )?,
            finite_f32(
                (self.b * offset_x + self.d * offset_y + self.f) * multiplier - f64::from(origin_y),
                "PNG image transform f",
            )?,
        );
        if !transform.is_valid() {
            return Err(FormatEncodeError::Encoding(
                "PNG image transform is not invertible".to_owned(),
            ));
        }
        Ok(transform)
    }
}

impl PngPlan {
    pub(super) fn new(
        scene: ResolvedScene,
        request: &RenderRequest,
        svg_trees: &SvgTreeCache,
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
                    elements.push(PngElement::planned(PlannedElement {
                        frame_clips: project_frame_clips(outline.frame_clips, scale)?,
                        transform: PngTransform::new(&outline.transform, scale),
                        opacity: outline.opacity,
                        link: outline.link,
                        width: outline.width,
                        height: outline.height,
                        kind: PngElementKind::RoughOutline {
                            source: outline.source,
                            drawing: outline.drawing,
                            stroke_color: outline.stroke_color,
                            fill_color: outline.fill_color,
                            stroke_width: outline.stroke_width,
                            stroke_style: outline.stroke_style,
                            source_stroke_width: outline.source_stroke_width,
                            background_color: outline.background_color,
                            label_mask: outline.label_mask.map(|mask| PngLabelMask {
                                x: mask.x * scale,
                                y: mask.y * scale,
                                width: mask.width * scale,
                                height: mask.height * scale,
                            }),
                            generated_label: outline.generated_label,
                            generated_label_color: outline.generated_label_color,
                            generated_label_offset_y: outline.generated_label_offset_y,
                            base_element_work_units: outline.base_element_work_units,
                        },
                    }));
                }
                ResolvedElement::Freedraw(freedraw) => {
                    elements.push(PngElement::planned(PlannedElement {
                        frame_clips: project_frame_clips(freedraw.frame_clips, scale)?,
                        transform: PngTransform::new(&freedraw.transform, scale),
                        opacity: freedraw.opacity,
                        link: freedraw.link,
                        width: freedraw.width,
                        height: freedraw.height,
                        kind: PngElementKind::Freedraw {
                            source: freedraw.source,
                            path: freedraw::project(freedraw.path)?,
                            background_fill: freedraw.background_fill,
                            color: freedraw.color,
                            background_color: freedraw.background_color,
                            source_stroke_width: freedraw.source_stroke_width,
                            base_element_work_units: freedraw.base_element_work_units,
                        },
                    }));
                }
                ResolvedElement::Frame(frame) => {
                    elements.push(PngElement::planned(PlannedElement {
                        frame_clips: project_frame_clips(frame.frame_clips, scale)?,
                        transform: PngTransform::new(&frame.transform, scale),
                        opacity: frame.opacity,
                        link: frame.link,
                        width: frame.width,
                        height: frame.height,
                        kind: PngElementKind::Frame {
                            source: frame.source,
                            name: frame.name.map(|name| PngFrameName {
                                transform: PngTransform::new(&name.transform, scale),
                                color: name.color,
                                label: name.label,
                            }),
                            outline: frame.outline,
                        },
                    }));
                }
                ResolvedElement::Text(text) => {
                    elements.push(PngElement::planned(PlannedElement {
                        frame_clips: project_frame_clips(text.frame_clips, scale)?,
                        transform: PngTransform::new(&text.transform, scale),
                        opacity: text.opacity,
                        link: text.link,
                        width: text.width,
                        height: text.height,
                        kind: PngElementKind::Text {
                            source: text.source,
                            text: text.text,
                            color: text.color,
                            base_element_work_units: text.base_element_work_units,
                        },
                    }));
                }
                ResolvedElement::Image(image) => {
                    elements.push(PngElement::planned(PlannedElement {
                        frame_clips: project_frame_clips(image.frame_clips, scale)?,
                        transform: PngTransform::new(&image.transform, scale),
                        opacity: image.opacity,
                        link: image.link,
                        width: image.width,
                        height: image.height,
                        kind: PngElementKind::Image {
                            source: image.source,
                            asset: image.asset,
                            dark_mode_filter: image.dark_mode_filter,
                            corner_clip: image.corner_clip.map(|clip| clip.path),
                            source_rect: image.source_rect,
                            scale: image.scale,
                        },
                    }));
                }
                ResolvedElement::ImageFallback(image) => {
                    // The resolved placeholder carries no link: PNG writes
                    // none, and the reference canvas draws its image
                    // placeholder without one.
                    elements.push(PngElement::planned(PlannedElement {
                        frame_clips: project_frame_clips(image.frame_clips, scale)?,
                        transform: PngTransform::new(&image.transform, scale),
                        opacity: image.opacity,
                        link: None,
                        width: image.width,
                        height: image.height,
                        kind: PngElementKind::ImageFallback {
                            source: image.source,
                            status: image.status,
                        },
                    }));
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
                    elements.push(PngElement::planned(PlannedElement {
                        frame_clips: project_frame_clips(fallback.frame_clips, scale)?,
                        transform: PngTransform::new(&fallback.transform, scale),
                        opacity: fallback.opacity,
                        link: fallback.link,
                        width: fallback.width,
                        height: fallback.height,
                        kind: PngElementKind::Fallback {
                            source: fallback.source,
                            label: fallback.label,
                            base_element_work_units: visual_commands * 32 + 8 + label_work,
                        },
                    }));
                }
            }
        }
        Ok(Self {
            width,
            height,
            background,
            svg_trees: svg_trees.clone(),
            fallback_colors: scene.fallback_colors,
            image_fallback_background: IMAGE_FALLBACK_BACKGROUND.for_mode(request.color_mode()),
            image_fallback_icon_color: IMAGE_FALLBACK_ICON_COLOR.for_mode(request.color_mode()),
            elements,
        })
    }

    /// Rasterizes the plan once and deflates the raster into the `IDAT`
    /// stream every artifact candidate is assembled from.
    pub(super) fn compress(&self) -> Result<PngIdatStream, FormatEncodeError> {
        let mut rgba = self.rasterize()?;
        canonicalize_straight_alpha(&mut rgba)?;
        compress_rgba(self.width, self.height, &rgba)
    }

    pub(super) fn mandatory_notices(&self) -> Result<Option<MandatoryNotices>, FormatEncodeError> {
        let has_image_fallback = self.elements.iter().any(|element| {
            element
                .painted_font_awesome_status(self.width, self.height)
                .is_some()
        });
        let has_error_image_fallback = self.elements.iter().any(|element| {
            element.painted_font_awesome_status(self.width, self.height)
                == Some(NormalizedImageStatus::Error)
        });
        let fallback_kinds: &[&str] = if has_error_image_fallback {
            &["image", "image-error"]
        } else {
            &["image"]
        };
        has_image_fallback
            .then(|| MandatoryNotices::for_fallbacks(fallback_kinds, OutputFormat::Png))
            .transpose()
            .map_err(|error| match error {
                NoticeError::Memory => FormatEncodeError::MemoryUnavailable {
                    subject_kind: "artifact_manifest",
                    subject: "font_awesome_notice_manifest",
                },
                NoticeError::InvalidProvenance => FormatEncodeError::Encoding(
                    "the mandatory Font Awesome notice could not be resolved".to_owned(),
                ),
            })
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
            let mut masks = PaintMasks::new(raster_width, raster_height, SUBPIXEL_SCALE);
            for element in &self.elements {
                if !element.render {
                    continue;
                }
                let paint_mask = masks.paint_mask(element)?;
                let PlannedElement {
                    frame_clips,
                    transform,
                    opacity,
                    link: _,
                    width,
                    height,
                    kind,
                } = &element.planned;
                match kind {
                    PngElementKind::RoughOutline {
                        source,
                        drawing,
                        stroke_color,
                        fill_color,
                        stroke_width,
                        stroke_style,
                        source_stroke_width,
                        background_color,
                        label_mask: _,
                        generated_label,
                        generated_label_color,
                        generated_label_offset_y,
                        base_element_work_units: _,
                    } => {
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG plan outline lost its source identity".to_owned(),
                            ));
                        }
                        let element_transform = *transform;
                        let transform = element_transform.raster_transform(SUBPIXEL_SCALE)?;
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
                            paint_mask,
                        )?;
                        for decoration in &drawing.decorations {
                            let fill =
                                decoration_fill_color(decoration, *stroke_color, *background_color);
                            let (decoration_style, decoration_stroke_width) =
                                decoration_stroke_style(decoration, *source_stroke_width);
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
                                paint_mask,
                            )?;
                        }
                        if let (Some(label), Some(color)) = (generated_label, generated_label_color)
                        {
                            let paint = solid_paint(*color, 1.0)?;
                            let label_transform =
                                element_transform.with_local_offset(0.0, *generated_label_offset_y);
                            for line in &label.lines {
                                let Some(path) = label_path(&line.path)? else {
                                    continue;
                                };
                                surface.fill_path(
                                    &path,
                                    &paint,
                                    FillRule::Winding,
                                    label_transform
                                        .text_line_transform(line.baseline, SUBPIXEL_SCALE)?,
                                    paint_mask,
                                );
                            }
                        }
                    }
                    PngElementKind::Freedraw {
                        source,
                        path,
                        background_fill,
                        color,
                        background_color,
                        source_stroke_width,
                        base_element_work_units: _,
                    } => {
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG freedraw lost its source identity".to_owned(),
                            ));
                        }
                        let raster_transform = transform.raster_transform(SUBPIXEL_SCALE)?;
                        if let (Some(fill), Some(background_color)) =
                            (background_fill, background_color)
                        {
                            let path = tiny_skia_path(&fill.path)?;
                            let paint = solid_paint(*background_color, *opacity)?;
                            match fill.kind {
                                FillKind::Solid => surface.fill_path(
                                    &path,
                                    &paint,
                                    FillRule::EvenOdd,
                                    raster_transform,
                                    paint_mask,
                                ),
                                FillKind::Sketch => {
                                    let stroke = Stroke {
                                        width: finite_f32(
                                            *source_stroke_width / 2.0,
                                            "PNG freedraw fill weight",
                                        )?,
                                        miter_limit: 4.0,
                                        line_cap: LineCap::Round,
                                        line_join: LineJoin::Round,
                                        dash: None,
                                    };
                                    surface.stroke_path(
                                        &path,
                                        &paint,
                                        &stroke,
                                        raster_transform,
                                        paint_mask,
                                    );
                                }
                            }
                        }
                        if path.is_empty() {
                            continue;
                        }
                        let path = tiny_skia_freedraw_path(path)?;
                        let paint = solid_paint(*color, *opacity)?;
                        surface.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            raster_transform,
                            paint_mask,
                        );
                    }
                    PngElementKind::Frame {
                        source,
                        name,
                        outline,
                    } => {
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG frame lost its source identity".to_owned(),
                            ));
                        }
                        if let Some(outline) = outline {
                            let path = tiny_skia_path(&outline.path)?;
                            // The reference's canvas path (`renderElement`)
                            // recolors a magic frame's outline by theme; its
                            // SVG path does not. Both are resolved for the mode.
                            let paint = solid_paint(outline.canvas_stroke_color, *opacity)?;
                            let stroke = Stroke {
                                width: FRAME_OUTLINE_STROKE_WIDTH as f32,
                                miter_limit: 4.0,
                                line_cap: LineCap::Round,
                                line_join: LineJoin::Round,
                                dash: None,
                            };
                            surface.stroke_path(
                                &path,
                                &paint,
                                &stroke,
                                transform.raster_transform(SUBPIXEL_SCALE)?,
                                paint_mask,
                            );
                        }
                        if let Some(name) = name {
                            if let Some(path) = label_path(&name.label.path)? {
                                let paint = solid_paint(name.color, 1.0)?;
                                surface.fill_path(
                                    &path,
                                    &paint,
                                    FillRule::Winding,
                                    name.transform
                                        .text_line_transform(name.label.baseline, SUBPIXEL_SCALE)?,
                                    None,
                                );
                            }
                        }
                    }
                    PngElementKind::Text {
                        source,
                        text,
                        color,
                        base_element_work_units: _,
                    } => {
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG text lost its source identity".to_owned(),
                            ));
                        }
                        let paint = solid_paint(*color, *opacity)?;
                        for line in &text.lines {
                            let Some(path) = label_path(&line.path)? else {
                                continue;
                            };
                            surface.fill_path(
                                &path,
                                &paint,
                                FillRule::Winding,
                                transform.text_line_transform(line.baseline, SUBPIXEL_SCALE)?,
                                paint_mask,
                            );
                        }
                    }
                    PngElementKind::Image {
                        source,
                        asset,
                        dark_mode_filter,
                        corner_clip: _,
                        source_rect,
                        scale,
                    } => {
                        // A degenerate destination paints no pixels. Avoid
                        // inverting its singular transform after a dimension
                        // default/recovery has legitimately produced zero.
                        if *width == 0.0 || *height == 0.0 {
                            continue;
                        }
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG image lost its source identity".to_owned(),
                            ));
                        }
                        // An axis-aligned raster image paints the whole device
                        // pixels whose centres its destination rectangle
                        // covers, as the canvas's non-antialiased `drawImage`
                        // does (#162); a rotated one keeps antialiased edges.
                        // Either way the corner clip and frame clips stay
                        // antialiased masks.
                        let snapped = element.snapped_image_rect(self.width, self.height);
                        if let Some(tree) = self.svg_trees.get(&asset.identity).map(Arc::as_ref) {
                            let corners = [
                                Point { x: 0.0, y: 0.0 },
                                Point { x: *width, y: 0.0 },
                                Point {
                                    x: *width,
                                    y: *height,
                                },
                                Point { x: 0.0, y: *height },
                            ]
                            .map(|point| transform.map_point(point));
                            let multiplier = f64::from(SUBPIXEL_SCALE);
                            let left = (corners
                                .iter()
                                .map(|point| point.x)
                                .fold(f64::INFINITY, f64::min)
                                * multiplier)
                                .floor()
                                .clamp(0.0, f64::from(raster_width))
                                as u32;
                            let top = (corners
                                .iter()
                                .map(|point| point.y)
                                .fold(f64::INFINITY, f64::min)
                                * multiplier)
                                .floor()
                                .clamp(0.0, f64::from(raster_height))
                                as u32;
                            let right = (corners
                                .iter()
                                .map(|point| point.x)
                                .fold(f64::NEG_INFINITY, f64::max)
                                * multiplier)
                                .ceil()
                                .clamp(0.0, f64::from(raster_width))
                                as u32;
                            let bottom = (corners
                                .iter()
                                .map(|point| point.y)
                                .fold(f64::NEG_INFINITY, f64::max)
                                * multiplier)
                                .ceil()
                                .clamp(0.0, f64::from(raster_height))
                                as u32;
                            let Some(layer_width) =
                                right.checked_sub(left).filter(|width| *width > 0)
                            else {
                                continue;
                            };
                            let Some(layer_height) =
                                bottom.checked_sub(top).filter(|height| *height > 0)
                            else {
                                continue;
                            };
                            let raster_transform = transform.raster_image_transform_with_origin(
                                SUBPIXEL_SCALE,
                                *source_rect,
                                *width,
                                *height,
                                *scale,
                                left,
                                top,
                            )?;
                            let mut layer = Pixmap::new(layer_width, layer_height).ok_or(
                                FormatEncodeError::MemoryUnavailable {
                                    subject_kind: "raster_surface",
                                    subject: "png_nested_svg_surface",
                                },
                            )?;
                            resvg::render(tree, raster_transform, &mut layer.as_mut());
                            if *dark_mode_filter == Some(DarkModeImageFilter::NestedSvgImage) {
                                // The reference canvas keeps its theme filter
                                // for a nested-SVG image (`shouldResetImageFilter`
                                // resets it only for rasters), so the viewer's
                                // light-mode pixels pass through it here.
                                apply_theme_filter(&mut layer);
                            }
                            if let Some(rect) = element.axis_aligned_device_rect() {
                                // The canvas antialiases the nested document's
                                // viewport clip like its content edge, so a
                                // fractional edge carries its coverage squared,
                                // and clips the whole draw to the enclosing
                                // element-space rectangle snapped to device
                                // pixels (#162).
                                let enclosing = transform.map_point(Point {
                                    x: width.ceil(),
                                    y: height.ceil(),
                                });
                                clip_nested_svg_like_the_canvas(
                                    &mut layer,
                                    left,
                                    top,
                                    rect,
                                    (
                                        rect.0,
                                        rect.1,
                                        rect.0.max(enclosing.x),
                                        rect.1.max(enclosing.y),
                                    ),
                                    SUBPIXEL_SCALE,
                                );
                            }
                            surface.draw_pixmap(
                                i32::try_from(left).map_err(|_| png_size_error())?,
                                i32::try_from(top).map_err(|_| png_size_error())?,
                                layer.as_ref(),
                                &PixmapPaint {
                                    opacity: finite_f32(*opacity, "PNG image opacity")?,
                                    ..PixmapPaint::default()
                                },
                                Transform::identity(),
                                paint_mask,
                            );
                            continue;
                        }
                        let raster_transform = transform.raster_image_transform(
                            SUBPIXEL_SCALE,
                            *source_rect,
                            *width,
                            *height,
                            *scale,
                        )?;
                        let pixels = asset.premultiplied_rgba.as_deref().ok_or_else(|| {
                            FormatEncodeError::Encoding(
                                "PNG image has no normalized pixel resource".to_owned(),
                            )
                        })?;
                        let pixmap = PixmapRef::from_bytes(pixels, asset.width, asset.height)
                            .ok_or_else(png_size_error)?;
                        let image_paint = PixmapPaint {
                            opacity: finite_f32(*opacity, "PNG image opacity")?,
                            quality: FilterQuality::Bilinear,
                            ..PixmapPaint::default()
                        };
                        match snapped {
                            Some(rect) => {
                                // The snapped rectangle, filled with the image
                                // resampled once per destination pixel centre
                                // as the canvas's `drawImage` resamples it
                                // (#165): a canvas-resolution layer of the
                                // rectangle's size takes the image as a
                                // pattern under the exact fractional mapping,
                                // sampled bilinearly at each of its pixel
                                // centres, and each of its pixels then paints
                                // one constant block of subpixels, which the
                                // box filter returns unchanged. Sampling the
                                // image at every subpixel centre instead gave
                                // an upscaled image a footprint one
                                // destination pixel wider than the canvas's.
                                // Coverage is the canvas's whole rows and
                                // columns, and the padded spread supplies the
                                // image's edge pixel where the snapped edge
                                // reaches past the mapped one, as Skia's
                                // clamped image shader does. A cropped image
                                // is sampled from the whole pixmap, as the
                                // canvas samples it: the official evidence for
                                // `focused.image-codecs.v1` blends the pixels
                                // beside the crop into its edge rows, so the
                                // source rectangle does not bound the sampling
                                // there either.
                                let layer_width = rect.right - rect.left;
                                let layer_height = rect.bottom - rect.top;
                                let sampling_transform = transform
                                    .raster_image_transform_with_origin(
                                        1,
                                        *source_rect,
                                        *width,
                                        *height,
                                        *scale,
                                        rect.left,
                                        rect.top,
                                    )?;
                                let mut layer = Pixmap::new(layer_width, layer_height).ok_or(
                                    FormatEncodeError::MemoryUnavailable {
                                        subject_kind: "raster_surface",
                                        subject: "png_image_resample_surface",
                                    },
                                )?;
                                let layer_rect = Rect::from_xywh(
                                    0.0,
                                    0.0,
                                    finite_f32(f64::from(layer_width), "PNG image layer width")?,
                                    finite_f32(f64::from(layer_height), "PNG image layer height")?,
                                )
                                .ok_or_else(png_size_error)?;
                                layer.fill_rect(
                                    layer_rect,
                                    &tiny_skia::Paint {
                                        shader: tiny_skia::Pattern::new(
                                            pixmap,
                                            tiny_skia::SpreadMode::Pad,
                                            image_paint.quality,
                                            1.0,
                                            sampling_transform,
                                        ),
                                        blend_mode: tiny_skia::BlendMode::Source,
                                        anti_alias: false,
                                        force_hq_pipeline: false,
                                    },
                                    Transform::identity(),
                                    None,
                                );
                                let scaled = |edge: u32| {
                                    finite_f32(
                                        f64::from(edge) * f64::from(SUBPIXEL_SCALE),
                                        "PNG image destination edge",
                                    )
                                };
                                let destination = Rect::from_ltrb(
                                    scaled(rect.left)?,
                                    scaled(rect.top)?,
                                    scaled(rect.right)?,
                                    scaled(rect.bottom)?,
                                )
                                .ok_or_else(png_size_error)?;
                                let block_scale =
                                    finite_f32(f64::from(SUBPIXEL_SCALE), "PNG image block scale")?;
                                surface.fill_rect(
                                    destination,
                                    &tiny_skia::Paint {
                                        shader: tiny_skia::Pattern::new(
                                            layer.as_ref(),
                                            tiny_skia::SpreadMode::Pad,
                                            FilterQuality::Nearest,
                                            image_paint.opacity,
                                            Transform::from_row(
                                                block_scale,
                                                0.0,
                                                0.0,
                                                block_scale,
                                                scaled(rect.left)?,
                                                scaled(rect.top)?,
                                            ),
                                        ),
                                        blend_mode: image_paint.blend_mode,
                                        anti_alias: false,
                                        force_hq_pipeline: false,
                                    },
                                    Transform::identity(),
                                    paint_mask,
                                );
                            }
                            None => surface.draw_pixmap(
                                0,
                                0,
                                pixmap,
                                &image_paint,
                                raster_transform,
                                paint_mask,
                            ),
                        }
                    }
                    PngElementKind::ImageFallback { source, status } => {
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG image fallback lost its source identity".to_owned(),
                            ));
                        }
                        let rect = Rect::from_xywh(
                            0.0,
                            0.0,
                            finite_f32(*width, "PNG image fallback width")?,
                            finite_f32(*height, "PNG image fallback height")?,
                        )
                        .ok_or_else(png_size_error)?;
                        surface.fill_path(
                            &PathBuilder::from_rect(rect),
                            &solid_paint(self.image_fallback_background, *opacity)?,
                            FillRule::Winding,
                            transform.raster_transform(SUBPIXEL_SCALE)?,
                            paint_mask,
                        );
                        let size = width.min(*height).min(width.min(*height) * 0.4).min(100.0);
                        if size <= 0.0 {
                            continue;
                        }
                        let icon_transform =
                            transform.raster_transform(SUBPIXEL_SCALE)?.pre_translate(
                                finite_f32((*width - size) / 2.0, "PNG image fallback icon x")?,
                                finite_f32((*height - size) / 2.0, "PNG image fallback icon y")?,
                            );
                        let icon_paint = solid_paint(self.image_fallback_icon_color, *opacity)?;
                        match status {
                            NormalizedImageStatus::Error => {
                                let icon_transform = icon_transform.pre_scale(
                                    finite_f32(size / 668.0, "PNG error icon scale")?,
                                    finite_f32(size / 668.0, "PNG error icon scale")?,
                                );
                                let image = svg_path(OFFICIAL_IMAGE_ERROR_PATH)?;
                                surface.fill_path(
                                    &image,
                                    &icon_paint,
                                    FillRule::Winding,
                                    icon_transform.pre_concat(Transform::from_row(
                                        0.81709, 0.0, 0.0, 0.81709, 124.825, 145.825,
                                    )),
                                    paint_mask,
                                );
                                let ban = svg_path(OFFICIAL_IMAGE_ERROR_BAN_PATH)?;
                                surface.fill_path(
                                    &ban,
                                    &icon_paint,
                                    FillRule::Winding,
                                    icon_transform.pre_concat(Transform::from_row(
                                        0.30366, 0.0, 0.0, 0.30366, 506.822, 60.065,
                                    )),
                                    paint_mask,
                                );
                            }
                            NormalizedImageStatus::Pending | NormalizedImageStatus::Saved => {
                                let icon = svg_path(OFFICIAL_IMAGE_PATH)?;
                                surface.fill_path(
                                    &icon,
                                    &icon_paint,
                                    FillRule::Winding,
                                    icon_transform.pre_scale(
                                        finite_f32(size / 512.0, "PNG image icon scale")?,
                                        finite_f32(size / 512.0, "PNG image icon scale")?,
                                    ),
                                    paint_mask,
                                );
                            }
                        }
                    }
                    PngElementKind::Fallback {
                        source,
                        label,
                        base_element_work_units: _,
                    } => {
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
                        for frame_clip in frame_clips {
                            let frame_bounds = tiny_skia_path(&frame_clip.path)?;
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
                            if let Some(label_path) = label_path(&label.path)? {
                                let label_paint = solid_paint(self.fallback_colors.label, 1.0)?;
                                layer.fill_path(
                                    &label_path,
                                    &label_paint,
                                    FillRule::Winding,
                                    transform,
                                    Some(&clip),
                                );
                            }
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

    /// The conservative device-space area of the element's own rectangle
    /// (`width` by `height` under its transform), clipped to the canvas.
    fn element_rect_coverage_area(&self, element: &PngElement) -> u64 {
        conservative_fallback_coverage_area(
            element.planned.width,
            element.planned.height,
            element.planned.transform,
            self.width,
            self.height,
        )
    }

    pub(super) fn vector_coverage_area(&self, index: usize) -> Option<u64> {
        self.elements.get(index).map(|element| {
            let planned = &element.planned;
            match &planned.kind {
                PngElementKind::RoughOutline {
                    drawing,
                    stroke_width,
                    ..
                } => conservative_coverage_area(
                    drawing,
                    planned.transform,
                    *stroke_width,
                    self.width,
                    self.height,
                ),
                PngElementKind::Freedraw {
                    path,
                    background_fill,
                    source_stroke_width,
                    ..
                } => conservative_freedraw_coverage_area(
                    path,
                    background_fill.as_ref(),
                    planned.transform,
                    *source_stroke_width,
                    self.width,
                    self.height,
                ),
                PngElementKind::Frame { .. } => 0,
                PngElementKind::Text { text, .. } => conservative_text_coverage_area(
                    text,
                    &planned.frame_clips,
                    planned.transform,
                    self.width,
                    self.height,
                ),
                PngElementKind::Image { .. } => 0,
                PngElementKind::ImageFallback { .. } | PngElementKind::Fallback { .. } => {
                    self.element_rect_coverage_area(element)
                }
            }
        })
    }

    pub(super) fn image_projection_area(&self, index: usize) -> Option<Option<u64>> {
        self.elements
            .get(index)
            .map(|element| match &element.planned.kind {
                PngElementKind::Image { .. } => Some(self.element_rect_coverage_area(element)),
                _ => None,
            })
    }

    pub(super) fn base_element_work_units(&self, index: usize) -> Option<u64> {
        self.elements
            .get(index)
            .map(|element| match &element.planned.kind {
                PngElementKind::RoughOutline {
                    base_element_work_units,
                    ..
                }
                | PngElementKind::Freedraw {
                    base_element_work_units,
                    ..
                }
                | PngElementKind::Fallback {
                    base_element_work_units,
                    ..
                } => *base_element_work_units,
                PngElementKind::Frame { .. }
                | PngElementKind::Image { .. }
                | PngElementKind::ImageFallback { .. } => 8,
                PngElementKind::Text {
                    base_element_work_units,
                    ..
                } => base_element_work_units.saturating_add(8),
            })
    }

    pub(super) fn fallback_surface_area(&self, index: usize) -> Option<Option<u64>> {
        self.elements
            .get(index)
            .map(|element| match &element.planned.kind {
                PngElementKind::Fallback { .. } => Some(self.element_rect_coverage_area(element)),
                PngElementKind::RoughOutline { .. }
                | PngElementKind::Freedraw { .. }
                | PngElementKind::Frame { .. }
                | PngElementKind::Text { .. }
                | PngElementKind::Image { .. }
                | PngElementKind::ImageFallback { .. } => None,
            })
    }

    /// The alpha mask an element needs beyond its shared frame clip mask, in
    /// canvas pixels: a bound-label exclusion or an image's own clip rectangle.
    /// `None` when the element paints through at most the shared frame mask.
    pub(super) fn paint_mask_surface_area(&self, index: usize) -> Option<Option<u64>> {
        self.elements
            .get(index)
            .map(|element| match &element.planned.kind {
                PngElementKind::RoughOutline {
                    label_mask: Some(label_mask),
                    ..
                } => Some(label_mask_coverage_area(
                    *label_mask,
                    self.width,
                    self.height,
                )),
                PngElementKind::Image { .. } | PngElementKind::ImageFallback { .. } => {
                    Some(self.element_rect_coverage_area(element))
                }
                _ => None,
            })
    }

    pub(super) fn has_paint_masks(&self) -> bool {
        self.elements.iter().any(|element| {
            matches!(
                element.planned.kind,
                PngElementKind::RoughOutline {
                    label_mask: Some(_),
                    ..
                } | PngElementKind::Image { .. }
                    | PngElementKind::ImageFallback { .. }
            )
        })
    }

    pub(super) fn maximum_paint_mask_surface_area(&self) -> u64 {
        self.elements
            .iter()
            .enumerate()
            .filter_map(|(index, _)| self.paint_mask_surface_area(index).flatten())
            .max()
            .unwrap_or(0)
    }

    /// The distinct frame clip masks this plan shares across clipped elements,
    /// in first-use element order. Each is charged once for the whole plan.
    pub(super) fn frame_clip_masks(&self) -> Result<Vec<PngFrameClipMask>, FormatEncodeError> {
        let mut representatives: Vec<&[PngFrameClip]> = Vec::new();
        for element in &self.elements {
            let clips = element.frame_clips();
            if clips.is_empty()
                || representatives
                    .iter()
                    .any(|known| same_frame_clip_set(known, clips))
            {
                continue;
            }
            representatives
                .try_reserve(1)
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_plan",
                    subject: "png_frame_clip_masks",
                })?;
            representatives.push(clips);
        }
        let mut masks = Vec::new();
        masks
            .try_reserve_exact(representatives.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "png_frame_clip_masks",
            })?;
        for clips in representatives {
            let frame_source_index = clips.first().map(|clip| clip.frame_source_index);
            masks.push(PngFrameClipMask {
                frame_element_index: self.elements.iter().position(|element| {
                    matches!(&element.planned.kind, PngElementKind::Frame { source, .. } if Some(source.source_index()) == frame_source_index)
                }),
                pixels: frame_clip_coverage_area(clips, self.width, self.height),
            });
        }
        Ok(masks)
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
        self.elements
            .get(index)
            .map(|element| match &element.planned.kind {
                PngElementKind::RoughOutline { source, .. }
                | PngElementKind::Freedraw { source, .. }
                | PngElementKind::Frame { source, .. }
                | PngElementKind::Text { source, .. }
                | PngElementKind::Image { source, .. }
                | PngElementKind::ImageFallback { source, .. } => {
                    (Some(source.id()), source.pointer())
                }
                PngElementKind::Fallback { source, .. } => {
                    (source.id.as_deref(), source.pointer.as_str())
                }
            })
    }

    /// Stops painting the element at `index`, keeping its plan. `false` when
    /// there is no such element.
    pub(super) fn omit_element(&mut self, index: usize) -> bool {
        match self.elements.get_mut(index) {
            Some(element) => {
                element.render = false;
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

fn pixel_dimension(value: f64) -> Result<u32, FormatEncodeError> {
    if !value.is_finite() || value < 1.0 || value >= (u32::MAX as f64) + 1.0 {
        return Err(FormatEncodeError::Encoding(
            "PNG dimensions must be finite positive 32-bit values".to_owned(),
        ));
    }
    Ok(value.trunc() as u32)
}
