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

use coverage::{
    conservative_coverage_area, conservative_fallback_coverage_area,
    conservative_freedraw_coverage_area, conservative_text_coverage_area, fallback_coverage_bounds,
    frame_clip_coverage_area, frame_clip_coverage_bounds, label_mask_coverage_area,
};
use encoder::{canonicalize_straight_alpha, compress_rgba, png_size_error};
use paint::{
    apply_theme_filter, downsample_rgba, fallback_paths, finite_f32, label_path,
    paint_rough_drawing, solid_paint, svg_path, tiny_skia_freedraw_path, tiny_skia_path,
    PaintMasks, OFFICIAL_IMAGE_ERROR_BAN_PATH, OFFICIAL_IMAGE_ERROR_PATH, OFFICIAL_IMAGE_PATH,
};

use std::sync::Arc;

use crate::normalization::{NormalizedElementSource, NormalizedImageStatus};
use crate::{color::CanonicalColor, OutputFormat, RenderRequest};
use tiny_skia::{
    Color, FillRule, FilterQuality, IntSize, LineCap, LineJoin, Mask, PathBuilder, Pixmap,
    PixmapMut, PixmapPaint, PixmapRef, Rect, Stroke, StrokeDash, Transform,
};

use super::geometry::{Path, Point, FRAME_OUTLINE_STROKE_WIDTH};
use super::notices::{MandatoryNotices, NoticeError};
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

#[derive(Debug)]
enum PngElement {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
        background_color: CanonicalColor,
        label_mask: Option<PngLabelMask>,
        generated_label: Option<ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
        base_element_work_units: u64,
        render: bool,
    },
    Freedraw {
        source: NormalizedElementSource,
        path: freedraw::ProjectedPath,
        background_fill: Option<Fill>,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        color: CanonicalColor,
        background_color: Option<CanonicalColor>,
        source_stroke_width: f64,
        opacity: f64,
        base_element_work_units: u64,
        render: bool,
    },
    Frame {
        source: NormalizedElementSource,
        name: Option<PngFrameName>,
        outline: Option<ResolvedFrameOutline>,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        opacity: f64,
        render: bool,
    },
    Text {
        source: NormalizedElementSource,
        text: ResolvedText,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        color: CanonicalColor,
        opacity: f64,
        base_element_work_units: u64,
        render: bool,
    },
    Image {
        source: NormalizedElementSource,
        asset: Arc<ResolvedImageAsset>,
        /// The dark-mode filter the canvas applies to this image. The canvas
        /// resets its filter for a raster image, so only a nested-SVG image's
        /// rasterized pixels are lowered here.
        dark_mode_filter: Option<DarkModeImageFilter>,
        frame_clips: Vec<PngFrameClip>,
        /// The rounded-corner clip of an image with a roundness record, in
        /// element space; without one the image clips to its rectangle.
        corner_clip: Option<Path>,
        transform: PngTransform,
        width: f64,
        height: f64,
        source_rect: ResolvedImageSourceRect,
        scale: [i8; 2],
        opacity: f64,
        render: bool,
    },
    ImageFallback {
        source: NormalizedElementSource,
        status: NormalizedImageStatus,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        width: f64,
        height: f64,
        opacity: f64,
        render: bool,
    },
    Fallback {
        source: FallbackSource,
        label: Option<ResolvedLabel>,
        frame_clips: Vec<PngFrameClip>,
        width: f64,
        height: f64,
        transform: PngTransform,
        opacity: f64,
        base_element_work_units: u64,
        render: bool,
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
    fn painted_font_awesome_status(
        &self,
        surface_width: u32,
        surface_height: u32,
    ) -> Option<NormalizedImageStatus> {
        match self {
            Self::ImageFallback {
                status,
                frame_clips,
                transform,
                width,
                height,
                opacity,
                render: true,
                ..
            } if *width > 0.0 && *height > 0.0 && *opacity > 0.0 => {
                let (left, top, right, bottom) = fallback_coverage_bounds(
                    *width,
                    *height,
                    *transform,
                    surface_width,
                    surface_height,
                );
                let (clip_left, clip_top, clip_right, clip_bottom) =
                    frame_clip_coverage_bounds(frame_clips, surface_width, surface_height)?;
                let left = left.max(clip_left);
                let top = top.max(clip_top);
                let right = right.min(clip_right);
                let bottom = bottom.min(clip_bottom);
                (left < right && top < bottom).then_some(*status)
            }
            _ => None,
        }
    }

    fn frame_clips(&self) -> &[PngFrameClip] {
        match self {
            Self::RoughOutline { frame_clips, .. }
            | Self::Freedraw { frame_clips, .. }
            | Self::Frame { frame_clips, .. }
            | Self::Text { frame_clips, .. }
            | Self::Image { frame_clips, .. }
            | Self::ImageFallback { frame_clips, .. } => frame_clips,
            Self::Fallback { .. } => &[],
        }
    }

    fn renders(&self) -> bool {
        match self {
            Self::RoughOutline { render, .. }
            | Self::Freedraw { render, .. }
            | Self::Frame { render, .. }
            | Self::Text { render, .. }
            | Self::Image { render, .. }
            | Self::ImageFallback { render, .. }
            | Self::Fallback { render, .. } => *render,
        }
    }

    fn label_mask(&self) -> Option<PngLabelMask> {
        match self {
            Self::RoughOutline { label_mask, .. } => *label_mask,
            _ => None,
        }
    }

    /// The element-space clip an image or its placeholder paints through: the
    /// element rectangle, or an image's rounded-corner outline when it has a
    /// roundness record. The placeholder is never corner-clipped: the
    /// reference's `drawElementOnCanvas` image case calls `roundRect` and
    /// `clip` only before `drawImage`, not around `drawImagePlaceholder`.
    fn image_clip(&self) -> Option<PngImageClip<'_>> {
        match self {
            Self::Image {
                width,
                height,
                corner_clip,
                transform,
                ..
            } => Some(PngImageClip {
                width: *width,
                height: *height,
                corner_clip: corner_clip.as_ref(),
                transform: *transform,
            }),
            Self::ImageFallback {
                width,
                height,
                transform,
                ..
            } => Some(PngImageClip {
                width: *width,
                height: *height,
                corner_clip: None,
                transform: *transform,
            }),
            _ => None,
        }
    }
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

    fn with_local_offset(mut self, x: f64, y: f64) -> Self {
        self.e += self.a * x + self.c * y;
        self.f += self.b * x + self.d * y;
        self
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
                    elements.push(PngElement::RoughOutline {
                        source: outline.source,
                        drawing: outline.drawing,
                        frame_clips: project_frame_clips(outline.frame_clips, scale)?,
                        transform: PngTransform::new(&outline.transform, scale),
                        stroke_color: outline.stroke_color,
                        fill_color: outline.fill_color,
                        stroke_width: outline.stroke_width,
                        stroke_style: outline.stroke_style,
                        source_stroke_width: outline.source_stroke_width,
                        opacity: outline.opacity,
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
                        render: true,
                    });
                }
                ResolvedElement::Freedraw(freedraw) => {
                    elements.push(PngElement::Freedraw {
                        source: freedraw.source,
                        path: freedraw::project(freedraw.path)?,
                        background_fill: freedraw.background_fill,
                        frame_clips: project_frame_clips(freedraw.frame_clips, scale)?,
                        transform: PngTransform::new(&freedraw.transform, scale),
                        color: freedraw.color,
                        background_color: freedraw.background_color,
                        source_stroke_width: freedraw.source_stroke_width,
                        opacity: freedraw.opacity,
                        base_element_work_units: freedraw.base_element_work_units,
                        render: true,
                    });
                }
                ResolvedElement::Frame(frame) => {
                    elements.push(PngElement::Frame {
                        source: frame.source,
                        name: frame.name.map(|name| PngFrameName {
                            transform: PngTransform::new(&name.transform, scale),
                            color: name.color,
                            label: name.label,
                        }),
                        outline: frame.outline,
                        frame_clips: project_frame_clips(frame.frame_clips, scale)?,
                        transform: PngTransform::new(&frame.transform, scale),
                        opacity: frame.opacity,
                        render: true,
                    });
                }
                ResolvedElement::Text(text) => {
                    elements.push(PngElement::Text {
                        source: text.source,
                        text: text.text,
                        frame_clips: project_frame_clips(text.frame_clips, scale)?,
                        transform: PngTransform::new(&text.transform, scale),
                        color: text.color,
                        opacity: text.opacity,
                        base_element_work_units: text.base_element_work_units,
                        render: true,
                    });
                }
                ResolvedElement::Image(image) => {
                    elements.push(PngElement::Image {
                        source: image.source,
                        asset: image.asset,
                        dark_mode_filter: image.dark_mode_filter,
                        frame_clips: project_frame_clips(image.frame_clips, scale)?,
                        corner_clip: image.corner_clip.map(|clip| clip.path),
                        transform: PngTransform::new(&image.transform, scale),
                        width: image.width,
                        height: image.height,
                        source_rect: image.source_rect,
                        scale: image.scale,
                        opacity: image.opacity,
                        render: true,
                    });
                }
                ResolvedElement::ImageFallback(image) => {
                    elements.push(PngElement::ImageFallback {
                        source: image.source,
                        status: image.status,
                        frame_clips: project_frame_clips(image.frame_clips, scale)?,
                        transform: PngTransform::new(&image.transform, scale),
                        width: image.width,
                        height: image.height,
                        opacity: image.opacity,
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
                        frame_clips: project_frame_clips(fallback.frame_clips, scale)?,
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
                if !element.renders() {
                    continue;
                }
                let paint_mask = masks.paint_mask(element)?;
                match element {
                    PngElement::RoughOutline {
                        source,
                        drawing,
                        frame_clips: _,
                        transform,
                        stroke_color,
                        fill_color,
                        stroke_width,
                        stroke_style,
                        source_stroke_width,
                        opacity,
                        background_color,
                        label_mask: _,
                        generated_label,
                        generated_label_color,
                        generated_label_offset_y,
                        ..
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
                            let label_transform = element_transform
                                .with_local_offset(0.0, *generated_label_offset_y)
                                .raster_transform(SUBPIXEL_SCALE)?;
                            for line in &label.lines {
                                let Some(path) = label_path(&line.path)? else {
                                    continue;
                                };
                                surface.fill_path(
                                    &path,
                                    &paint,
                                    FillRule::Winding,
                                    label_transform,
                                    paint_mask,
                                );
                            }
                        }
                    }
                    PngElement::Freedraw {
                        source,
                        path,
                        background_fill,
                        frame_clips: _,
                        transform,
                        color,
                        background_color,
                        source_stroke_width,
                        opacity,
                        ..
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
                    PngElement::Frame {
                        source,
                        name,
                        outline,
                        frame_clips: _,
                        transform,
                        opacity,
                        render: _,
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
                                    name.transform.raster_transform(SUBPIXEL_SCALE)?,
                                    None,
                                );
                            }
                        }
                    }
                    PngElement::Text {
                        source,
                        text,
                        frame_clips: _,
                        transform,
                        color,
                        opacity,
                        base_element_work_units: _,
                        render: _,
                    } => {
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG text lost its source identity".to_owned(),
                            ));
                        }
                        let paint = solid_paint(*color, *opacity)?;
                        let raster_transform = transform.raster_transform(SUBPIXEL_SCALE)?;
                        for line in &text.lines {
                            let Some(path) = label_path(&line.path)? else {
                                continue;
                            };
                            surface.fill_path(
                                &path,
                                &paint,
                                FillRule::Winding,
                                raster_transform,
                                paint_mask,
                            );
                        }
                    }
                    PngElement::Image {
                        source,
                        asset,
                        dark_mode_filter,
                        transform,
                        width,
                        height,
                        source_rect,
                        scale,
                        opacity,
                        ..
                    } => {
                        if source.id().is_empty() || source.pointer().is_empty() {
                            return Err(FormatEncodeError::Encoding(
                                "PNG image lost its source identity".to_owned(),
                            ));
                        }
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
                        surface.draw_pixmap(
                            0,
                            0,
                            pixmap,
                            &PixmapPaint {
                                opacity: finite_f32(*opacity, "PNG image opacity")?,
                                quality: FilterQuality::Bilinear,
                                ..PixmapPaint::default()
                            },
                            raster_transform,
                            paint_mask,
                        );
                    }
                    PngElement::ImageFallback {
                        source,
                        status,
                        transform,
                        width,
                        height,
                        opacity,
                        ..
                    } => {
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
                    PngElement::Fallback {
                        source,
                        label,
                        frame_clips,
                        width,
                        height,
                        transform,
                        opacity,
                        ..
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
            PngElement::Freedraw {
                path,
                background_fill,
                transform,
                source_stroke_width,
                ..
            } => conservative_freedraw_coverage_area(
                path,
                background_fill.as_ref(),
                *transform,
                *source_stroke_width,
                self.width,
                self.height,
            ),
            PngElement::Frame { .. } => 0,
            PngElement::Text {
                text,
                frame_clips,
                transform,
                ..
            } => conservative_text_coverage_area(
                text,
                frame_clips,
                *transform,
                self.width,
                self.height,
            ),
            PngElement::Image { .. } => 0,
            PngElement::ImageFallback {
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

    pub(super) fn image_projection_area(&self, index: usize) -> Option<Option<u64>> {
        self.elements.get(index).map(|element| match element {
            PngElement::Image {
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
            _ => None,
        })
    }

    pub(super) fn base_element_work_units(&self, index: usize) -> Option<u64> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughOutline {
                base_element_work_units,
                ..
            } => *base_element_work_units,
            PngElement::Freedraw {
                base_element_work_units,
                ..
            } => *base_element_work_units,
            PngElement::Frame { .. } => 8,
            PngElement::Text {
                base_element_work_units,
                ..
            } => base_element_work_units.saturating_add(8),
            PngElement::Image { .. } => 8,
            PngElement::ImageFallback { .. } => 8,
            PngElement::Fallback {
                base_element_work_units,
                ..
            } => *base_element_work_units,
        })
    }

    pub(super) fn fallback_surface_area(&self, index: usize) -> Option<Option<u64>> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughOutline { .. } => None,
            PngElement::Freedraw { .. } => None,
            PngElement::Frame { .. } => None,
            PngElement::Text { .. } => None,
            PngElement::Image { .. } => None,
            PngElement::ImageFallback { .. } => None,
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

    /// The alpha mask an element needs beyond its shared frame clip mask, in
    /// canvas pixels: a bound-label exclusion or an image's own clip rectangle.
    /// `None` when the element paints through at most the shared frame mask.
    pub(super) fn paint_mask_surface_area(&self, index: usize) -> Option<Option<u64>> {
        self.elements.get(index).map(|element| match element {
            PngElement::RoughOutline {
                label_mask: Some(label_mask),
                ..
            } => Some(label_mask_coverage_area(
                *label_mask,
                self.width,
                self.height,
            )),
            PngElement::Image {
                width,
                height,
                transform,
                ..
            }
            | PngElement::ImageFallback {
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
            _ => None,
        })
    }

    pub(super) fn has_paint_masks(&self) -> bool {
        self.elements.iter().any(|element| {
            matches!(
                element,
                PngElement::RoughOutline {
                    label_mask: Some(_),
                    ..
                } | PngElement::Image { .. }
                    | PngElement::ImageFallback { .. }
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
                    matches!(element, PngElement::Frame { source, .. } if Some(source.source_index()) == frame_source_index)
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
        self.elements.get(index).map(|element| match element {
            PngElement::RoughOutline { source, .. } => (Some(source.id()), source.pointer()),
            PngElement::Freedraw { source, .. } => (Some(source.id()), source.pointer()),
            PngElement::Frame { source, .. } => (Some(source.id()), source.pointer()),
            PngElement::Text { source, .. } => (Some(source.id()), source.pointer()),
            PngElement::Image { source, .. } => (Some(source.id()), source.pointer()),
            PngElement::ImageFallback { source, .. } => (Some(source.id()), source.pointer()),
            PngElement::Fallback { source, .. } => (source.id.as_deref(), source.pointer.as_str()),
        })
    }

    pub(super) fn omit_element(&mut self, index: usize) -> bool {
        match self.elements.get_mut(index) {
            Some(PngElement::RoughOutline { render, .. }) => {
                *render = false;
                true
            }
            Some(PngElement::Freedraw { render, .. }) => {
                *render = false;
                true
            }
            Some(PngElement::Frame { render, .. }) => {
                *render = false;
                true
            }
            Some(PngElement::Text { render, .. }) => {
                *render = false;
                true
            }
            Some(PngElement::Image { render, .. }) => {
                *render = false;
                true
            }
            Some(PngElement::ImageFallback { render, .. }) => {
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

fn pixel_dimension(value: f64) -> Result<u32, FormatEncodeError> {
    if !value.is_finite() || value < 1.0 || value >= (u32::MAX as f64) + 1.0 {
        return Err(FormatEncodeError::Encoding(
            "PNG dimensions must be finite positive 32-bit values".to_owned(),
        ));
    }
    Ok(value.trunc() as u32)
}
