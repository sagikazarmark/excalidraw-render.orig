use crate::normalization::NormalizedElementSource;
use crate::{
    color::{self, CanonicalColor},
    RenderRequest,
};
use miniz_oxide::deflate::core::{compress_to_output, CompressorOxide, TDEFLFlush, TDEFLStatus};
use miniz_oxide::DataFormat;
use tiny_skia::{
    BlendMode, Color, ColorU8, FillRule, FilterQuality, IntSize, LineCap, LineJoin, Mask, Paint,
    PathBuilder, PathStroker, Pixmap, PixmapMut, PixmapPaint, PixmapRef, Rect, Shader, Stroke,
    StrokeDash, Transform,
};

use super::resolve::{DarkModeImageFilter, ResolvedElement, ResolvedScene, StrokeStyle};
use super::rough::{Drawing, FillKind, Path, PathOperation, Point};
use super::FormatEncodeError;

#[derive(Debug)]
pub(super) struct PngPlan {
    width: u32,
    height: u32,
    background: Option<[u8; 4]>,
    fallback_colors: super::resolve::FallbackColors,
    /// `drawImagePlaceholder`'s `#E7E7E7` rectangle, for the color mode.
    image_fallback_background: CanonicalColor,
    /// `drawImagePlaceholder`'s `#888` Font Awesome glyph, for the color mode.
    image_fallback_icon_color: CanonicalColor,
    svg_trees: super::SvgTreeCache,
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
        generated_label: Option<super::text::ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
        base_element_work_units: u64,
        render: bool,
    },
    Freedraw {
        source: NormalizedElementSource,
        path: super::freedraw::ProjectedPath,
        background_fill: Option<super::rough::Fill>,
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
        outline: Option<super::resolve::ResolvedFrameOutline>,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        opacity: f64,
        render: bool,
    },
    Text {
        source: NormalizedElementSource,
        text: super::text::ResolvedText,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        color: CanonicalColor,
        opacity: f64,
        base_element_work_units: u64,
        render: bool,
    },
    Image {
        source: NormalizedElementSource,
        asset: std::sync::Arc<super::resolve::ResolvedImageAsset>,
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
        source_rect: super::resolve::ResolvedImageSourceRect,
        scale: [i8; 2],
        opacity: f64,
        render: bool,
    },
    ImageFallback {
        source: NormalizedElementSource,
        status: crate::normalization::NormalizedImageStatus,
        frame_clips: Vec<PngFrameClip>,
        transform: PngTransform,
        width: f64,
        height: f64,
        opacity: f64,
        render: bool,
    },
    Fallback {
        source: super::resolve::FallbackSource,
        label: Option<super::text::ResolvedLabel>,
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
    label: super::text::ResolvedLabel,
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
    clips: Vec<super::resolve::ResolvedClip>,
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

/// The clipped conservative device-space integer bounds of a frame clip set on
/// a `surface_width` x `surface_height` surface, or `None` when the set clips
/// everything away.
fn frame_clip_coverage_bounds(
    clips: &[PngFrameClip],
    surface_width: u32,
    surface_height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let (mut left, mut top, mut right, mut bottom) = (0, 0, surface_width, surface_height);
    for clip in clips {
        let bounds = clip.path.cubic_bounds()?;
        let points = [
            Point {
                x: bounds.0,
                y: bounds.1,
            },
            Point {
                x: bounds.2,
                y: bounds.1,
            },
            Point {
                x: bounds.2,
                y: bounds.3,
            },
            Point {
                x: bounds.0,
                y: bounds.3,
            },
        ]
        .map(|point| clip.transform.map_point(point));
        let clip_left = points
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .clamp(0.0, f64::from(surface_width)) as u32;
        let clip_top = points
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .clamp(0.0, f64::from(surface_height)) as u32;
        let clip_right = points
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .clamp(0.0, f64::from(surface_width)) as u32;
        let clip_bottom = points
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .clamp(0.0, f64::from(surface_height)) as u32;
        left = left.max(clip_left);
        top = top.max(clip_top);
        right = right.min(clip_right);
        bottom = bottom.min(clip_bottom);
    }
    (left < right && top < bottom).then_some((left, top, right, bottom))
}

/// The clipped conservative device-space area of a frame clip set in canvas
/// pixels.
fn frame_clip_coverage_area(
    clips: &[PngFrameClip],
    surface_width: u32,
    surface_height: u32,
) -> u64 {
    frame_clip_coverage_bounds(clips, surface_width, surface_height).map_or(
        0,
        |(left, top, right, bottom)| {
            u64::from(right - left).saturating_mul(u64::from(bottom - top))
        },
    )
}

/// The clipped conservative device-space area of a bound-label exclusion in
/// canvas pixels.
fn label_mask_coverage_area(label: PngLabelMask, surface_width: u32, surface_height: u32) -> u64 {
    let left = label.x.floor().clamp(0.0, f64::from(surface_width)) as u32;
    let top = label.y.floor().clamp(0.0, f64::from(surface_height)) as u32;
    let right = (label.x + label.width)
        .ceil()
        .clamp(0.0, f64::from(surface_width)) as u32;
    let bottom = (label.y + label.height)
        .ceil()
        .clamp(0.0, f64::from(surface_height)) as u32;
    u64::from(right.saturating_sub(left)).saturating_mul(u64::from(bottom.saturating_sub(top)))
}

impl PngElement {
    fn painted_font_awesome_status(
        &self,
        surface_width: u32,
        surface_height: u32,
    ) -> Option<crate::normalization::NormalizedImageStatus> {
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
    fn new(transform: &super::resolve::ResolvedTransform, scale: f64) -> Self {
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
        source: super::resolve::ResolvedImageSourceRect,
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
        source: super::resolve::ResolvedImageSourceRect,
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
        svg_trees: &super::SvgTreeCache,
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
                        path: super::freedraw::project(freedraw.path)?,
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

    pub(super) fn mandatory_notices(
        &self,
    ) -> Result<Option<super::notices::MandatoryNotices>, FormatEncodeError> {
        let has_image_fallback = self.elements.iter().any(|element| {
            element
                .painted_font_awesome_status(self.width, self.height)
                .is_some()
        });
        let has_error_image_fallback = self.elements.iter().any(|element| {
            element.painted_font_awesome_status(self.width, self.height)
                == Some(crate::normalization::NormalizedImageStatus::Error)
        });
        let fallback_kinds: &[&str] = if has_error_image_fallback {
            &["image", "image-error"]
        } else {
            &["image"]
        };
        has_image_fallback
            .then(|| {
                super::notices::MandatoryNotices::for_fallbacks(
                    fallback_kinds,
                    crate::OutputFormat::Png,
                )
            })
            .transpose()
            .map_err(|error| match error {
                super::notices::NoticeError::Memory => FormatEncodeError::MemoryUnavailable {
                    subject_kind: "artifact_manifest",
                    subject: "font_awesome_notice_manifest",
                },
                super::notices::NoticeError::InvalidProvenance => FormatEncodeError::Encoding(
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
                            let fill = super::resolve::decoration_fill_color(
                                decoration,
                                *stroke_color,
                                *background_color,
                            );
                            let (decoration_style, decoration_stroke_width) =
                                super::resolve::decoration_stroke_style(
                                    decoration,
                                    *source_stroke_width,
                                );
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
                                width: super::geometry::FRAME_OUTLINE_STROKE_WIDTH as f32,
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
                        if let Some(tree) = self
                            .svg_trees
                            .get(&asset.identity)
                            .map(std::sync::Arc::as_ref)
                        {
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
                            crate::normalization::NormalizedImageStatus::Error => {
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
                            crate::normalization::NormalizedImageStatus::Pending
                            | crate::normalization::NormalizedImageStatus::Saved => {
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

/// Bytes per RGBA sample; the filter distance to the left neighbour.
const PIXEL_BYTES: usize = 4;

/// The zlib default. Higher levels cost two to three times the encode time for
/// under two percent on measured Excalidraw boards, and the reference renderer's
/// own PNG export runs at this level.
const IDAT_DEFLATE_LEVEL: u8 = 6;

/// One raster's `IDAT` stream, filtered and deflated once, from which every
/// artifact candidate is assembled.
///
/// The stream is a pure function of the raster and independent of the
/// metadata chunks, so metadata selection assembles each candidate and the
/// emission pass assembles the accepted one from this single stream rather
/// than rasterizing and compressing again per call (#117). It is a transient
/// the ledger does not price separately; ADR-0004 documents its lifetime
/// against the spec §13.2 PNG encoder workspace row.
pub(super) struct PngIdatStream {
    width: u32,
    height: u32,
    idat: Vec<u8>,
}

/// Validates the raster against its dimensions and deflates its scanlines.
fn compress_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<PngIdatStream, FormatEncodeError> {
    if width == 0 || height == 0 {
        return Err(FormatEncodeError::Encoding(
            "PNG dimensions must be positive".to_owned(),
        ));
    }
    let pixel_row_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(PIXEL_BYTES))
        .ok_or_else(png_size_error)?;
    let expected_rgba_bytes = pixel_row_bytes
        .checked_mul(usize::try_from(height).map_err(|_| png_size_error())?)
        .ok_or_else(png_size_error)?;
    if rgba.len() != expected_rgba_bytes {
        return Err(FormatEncodeError::Encoding(
            "PNG raster length does not match its dimensions".to_owned(),
        ));
    }

    // The IDAT stream is produced before any candidate is assembled so the
    // exact PNG size is known before each fallible output allocation. Deflate
    // keeps the stream a small fraction of the raster on ordinary boards
    // (#113); even an incompressible raster holds it only until the accepted
    // candidate is emitted.
    let idat = compress_scanlines(pixel_row_bytes, rgba)?;
    Ok(PngIdatStream {
        width,
        height,
        idat,
    })
}

impl PngIdatStream {
    /// Assembles `IHDR`, the `IDAT` stream, the metadata chunks, and `IEND`
    /// into one artifact candidate.
    pub(super) fn assemble(
        &self,
        metadata: Option<&super::metadata::ArtifactMetadata>,
    ) -> Result<Vec<u8>, FormatEncodeError> {
        let idat = &self.idat;
        let metadata_bytes = metadata.map_or(0, |metadata| {
            metadata
                .scene_wrapper()
                .map_or(0, |wrapper| {
                    12_usize
                        .saturating_add("application/vnd.excalidraw+json".len())
                        .saturating_add(1)
                        .saturating_add(wrapper.len())
                })
                .saturating_add(metadata.manifest().map_or(0, |manifest| {
                    12_usize
                        .saturating_add("excalidraw-render:artifact-manifest".len())
                        .saturating_add(5)
                        .saturating_add(manifest.len())
                }))
        });
        let output_bytes = 8_usize
            .checked_add(12 + 13)
            .and_then(|bytes| bytes.checked_add(12))
            .and_then(|bytes| bytes.checked_add(idat.len()))
            .and_then(|bytes| bytes.checked_add(metadata_bytes))
            .and_then(|bytes| bytes.checked_add(12))
            .ok_or_else(png_size_error)?;

        let mut png = Vec::new();
        png.try_reserve_exact(output_bytes)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "artifact",
                subject: "png_output",
            })?;
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

        let mut ihdr = [0_u8; 13];
        ihdr[..4].copy_from_slice(&self.width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&self.height.to_be_bytes());
        ihdr[8] = 8;
        ihdr[9] = 6;
        append_chunk(&mut png, *b"IHDR", &ihdr)?;
        append_chunk(&mut png, *b"IDAT", idat)?;
        if let Some(metadata) = metadata {
            if let Some(wrapper) = metadata.scene_wrapper() {
                let mut text = Vec::new();
                text.try_reserve_exact(
                    "application/vnd.excalidraw+json"
                        .len()
                        .saturating_add(1)
                        .saturating_add(wrapper.len()),
                )
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "metadata",
                    subject: "png_scene_chunk",
                })?;
                text.extend_from_slice(b"application/vnd.excalidraw+json\0");
                text.extend_from_slice(wrapper);
                append_chunk(&mut png, *b"tEXt", &text).map_err(|_| {
                    super::metadata_embedding_error(
                        crate::DiagnosticCode::METADATA_SCENE_EMBEDDING_FAILED,
                        crate::MetadataComponent::EmbeddedScene,
                    )
                })?;
            }
            if let Some(manifest) = metadata.manifest() {
                let mut text = Vec::new();
                text.try_reserve_exact(
                    "excalidraw-render:artifact-manifest"
                        .len()
                        .saturating_add(5)
                        .saturating_add(manifest.len()),
                )
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "metadata",
                    subject: "png_manifest_chunk",
                })?;
                text.extend_from_slice(b"excalidraw-render:artifact-manifest\0\0\0\0\0");
                text.extend_from_slice(manifest.as_bytes());
                append_chunk(&mut png, *b"iTXt", &text).map_err(|_| {
                    super::metadata_embedding_error(
                        crate::DiagnosticCode::METADATA_MANIFEST_EMBEDDING_FAILED,
                        crate::MetadataComponent::Manifest,
                    )
                })?;
            }
        }
        append_chunk(&mut png, *b"IEND", &[])?;

        if png.len() != output_bytes {
            return Err(FormatEncodeError::Encoding(
                "PNG encoder length invariant failed".to_owned(),
            ));
        }
        Ok(png)
    }
}

/// Filters and deflates every scanline into one RFC 1950 zlib stream.
///
/// The compressor is pure integer code with fixed parameters and no host,
/// time, or random input, so the stream bytes are stable across every
/// supported target; the `IDAT` bytes of the package-consumer expectation
/// pin that on each of them.
fn compress_scanlines(pixel_row_bytes: usize, rgba: &[u8]) -> Result<Vec<u8>, FormatEncodeError> {
    if pixel_row_bytes == 0 {
        return Err(png_size_error());
    }
    let scanline_bytes = pixel_row_bytes.checked_add(1).ok_or_else(png_size_error)?;
    let mut scanline = zeroed_scratch(scanline_bytes, "png_scanline")?;
    // RFC 2083 filters treat the row above the first scanline as zeros.
    let zero_row = zeroed_scratch(pixel_row_bytes, "png_scanline_above")?;

    let mut compressor = CompressorOxide::default();
    compressor.set_format_and_level(DataFormat::Zlib, IDAT_DEFLATE_LEVEL);
    let mut zlib = Vec::new();
    let mut previous: &[u8] = &zero_row;
    for row in rgba.chunks_exact(pixel_row_bytes) {
        let Some((type_byte, residuals)) = scanline.split_first_mut() else {
            return Err(FormatEncodeError::Encoding(
                "PNG scanline layout invariant failed".to_owned(),
            ));
        };
        let filter = select_scanline_filter(row, previous);
        *type_byte = filter.type_byte();
        apply_scanline_filter(filter, row, previous, residuals);
        deflate_append(&mut compressor, &scanline, TDEFLFlush::None, &mut zlib)?;
        previous = row;
    }
    deflate_append(&mut compressor, &[], TDEFLFlush::Finish, &mut zlib)?;
    Ok(zlib)
}

fn zeroed_scratch(bytes: usize, subject: &'static str) -> Result<Vec<u8>, FormatEncodeError> {
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(bytes)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "artifact",
            subject,
        })?;
    scratch.resize(bytes, 0);
    Ok(scratch)
}

/// Feeds `input` through the compressor, appending its output fallibly.
fn deflate_append(
    compressor: &mut CompressorOxide,
    input: &[u8],
    flush: TDEFLFlush,
    output: &mut Vec<u8>,
) -> Result<(), FormatEncodeError> {
    let mut memory_unavailable = false;
    let (status, consumed) = compress_to_output(compressor, input, flush, |bytes| {
        if output.try_reserve(bytes.len()).is_err() {
            memory_unavailable = true;
            return false;
        }
        output.extend_from_slice(bytes);
        true
    });
    if memory_unavailable {
        return Err(FormatEncodeError::MemoryUnavailable {
            subject_kind: "artifact",
            subject: "png_idat",
        });
    }
    let expected = if flush == TDEFLFlush::Finish {
        TDEFLStatus::Done
    } else {
        TDEFLStatus::Okay
    };
    if status != expected || consumed != input.len() {
        return Err(FormatEncodeError::Encoding(
            "PNG deflate stream invariant failed".to_owned(),
        ));
    }
    Ok(())
}

/// RFC 2083 §6 scanline filter types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanlineFilter {
    None,
    Sub,
    Up,
    Average,
    Paeth,
}

impl ScanlineFilter {
    const ALL: [Self; 5] = [Self::None, Self::Sub, Self::Up, Self::Average, Self::Paeth];

    fn type_byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Sub => 1,
            Self::Up => 2,
            Self::Average => 3,
            Self::Paeth => 4,
        }
    }

    /// The filtered value of `row[index]`.
    fn residual(self, index: usize, row: &[u8], previous: &[u8]) -> u8 {
        row.get(index)
            .copied()
            .unwrap_or(0)
            .wrapping_sub(self.predict(index, row, previous))
    }

    /// The predicted value of `row[index]` from its left, upper, and
    /// upper-left neighbours, each zero where it falls outside the image.
    fn predict(self, index: usize, row: &[u8], previous: &[u8]) -> u8 {
        let left = index.checked_sub(PIXEL_BYTES);
        let a = left.and_then(|left| row.get(left)).copied().unwrap_or(0);
        let b = previous.get(index).copied().unwrap_or(0);
        let c = left
            .and_then(|left| previous.get(left))
            .copied()
            .unwrap_or(0);
        match self {
            Self::None => 0,
            Self::Sub => a,
            Self::Up => b,
            // floor((a + b) / 2) without leaving u8.
            Self::Average => a / 2 + b / 2 + (a & b & 1),
            Self::Paeth => paeth_predictor(a, b, c),
        }
    }
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let estimate = i16::from(a) + i16::from(b) - i16::from(c);
    let to_a = (estimate - i16::from(a)).abs();
    let to_b = (estimate - i16::from(b)).abs();
    let to_c = (estimate - i16::from(c)).abs();
    if to_a <= to_b && to_a <= to_c {
        a
    } else if to_b <= to_c {
        b
    } else {
        c
    }
}

/// Chooses the filter for one scanline.
///
/// Anti-aliased vector content is mostly flat runs and repeated rows, which
/// deflate matches directly; every predictive filter measured 10-20% larger
/// on real boards. A row whose bytes mostly equal their left or upper
/// neighbour therefore stays unfiltered. Other rows (photographic image
/// content) take the filter with the smallest sum of absolute residuals,
/// which halves their size; ties go to the lowest filter type.
fn select_scanline_filter(row: &[u8], previous: &[u8]) -> ScanlineFilter {
    let neighbour_matches = row
        .iter()
        .zip(previous)
        .enumerate()
        .filter(|(index, (byte, above))| {
            let left = index
                .checked_sub(PIXEL_BYTES)
                .and_then(|left| row.get(left));
            byte == above || left == Some(byte)
        })
        .count();
    if neighbour_matches.saturating_mul(2) >= row.len() {
        return ScanlineFilter::None;
    }

    let mut scores = [0_u64; 5];
    for index in 0..row.len() {
        for (filter, score) in ScanlineFilter::ALL.iter().zip(scores.iter_mut()) {
            let residual = filter.residual(index, row, previous);
            *score = score.saturating_add(u64::from(residual_magnitude(residual)));
        }
    }
    let mut best = ScanlineFilter::None;
    let mut best_score = u64::MAX;
    for (filter, score) in ScanlineFilter::ALL.into_iter().zip(scores) {
        if score < best_score {
            best = filter;
            best_score = score;
        }
    }
    best
}

/// The magnitude of a residual read as a signed byte, so 255 counts as 1.
fn residual_magnitude(residual: u8) -> u8 {
    if residual > 127 {
        0_u8.wrapping_sub(residual)
    } else {
        residual
    }
}

/// Writes the filtered residuals of `row` into `residuals`.
fn apply_scanline_filter(
    filter: ScanlineFilter,
    row: &[u8],
    previous: &[u8],
    residuals: &mut [u8],
) {
    for (index, residual) in residuals.iter_mut().enumerate().take(row.len()) {
        *residual = filter.residual(index, row, previous);
    }
}

/// Lowers the reference canvas's theme filter into a rasterized layer: each
/// pixel's unpremultiplied color passes through the filter and is premultiplied
/// again, as the canvas filter reads and writes its surface; transparent pixels
/// have no color to filter.
fn apply_theme_filter(layer: &mut Pixmap) {
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

fn tiny_skia_freedraw_path(
    path: &super::freedraw::ProjectedPath,
) -> Result<tiny_skia::Path, FormatEncodeError> {
    let points = path
        .operations
        .len()
        .checked_mul(2)
        .ok_or_else(png_size_error)?;
    let mut builder = PathBuilder::with_capacity(path.operations.len(), points);
    for operation in &path.operations {
        match operation {
            super::freedraw::PathOperation::Move(point) => builder.move_to(
                finite_f32(point.x, "PNG freedraw x")?,
                finite_f32(point.y, "PNG freedraw y")?,
            ),
            super::freedraw::PathOperation::Quadratic { control, end } => builder.quad_to(
                finite_f32(control.x, "PNG freedraw control x")?,
                finite_f32(control.y, "PNG freedraw control y")?,
                finite_f32(end.x, "PNG freedraw x")?,
                finite_f32(end.y, "PNG freedraw y")?,
            ),
            super::freedraw::PathOperation::Line(point) => builder.line_to(
                finite_f32(point.x, "PNG freedraw x")?,
                finite_f32(point.y, "PNG freedraw y")?,
            ),
            super::freedraw::PathOperation::Close => builder.close(),
        }
    }
    builder.finish().ok_or_else(|| {
        FormatEncodeError::Encoding("PNG freedraw path contains no outline".to_owned())
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

/// The raster path for one laid-out label line, or `None` when the line has no
/// outline to paint: an empty line, a whitespace-only line, or one whose glyphs
/// all lack outlines. Such a line still occupies its line step; it just paints
/// nothing, as in SVG and PDF (#111).
fn label_path(path: &super::text::LabelPath) -> Result<Option<tiny_skia::Path>, FormatEncodeError> {
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
    // `finish` declines a lone move, which has nothing to fill either.
    Ok(builder.finish())
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
struct PaintMasks {
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
    fn new(width: u32, height: u32, multiplier: u32) -> Self {
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
    fn paint_mask(&mut self, element: &PngElement) -> Result<Option<&Mask>, FormatEncodeError> {
        let clips = element.frame_clips();
        let label_mask = element.label_mask();
        let image_clip = element.image_clip();
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

const OFFICIAL_IMAGE_PATH: &str = "M464 448H48c-26.51 0-48-21.49-48-48V112c0-26.51 21.49-48 48-48h416c26.51 0 48 21.49 48 48v288c0 26.51-21.49 48-48 48zM112 120c-30.928 0-56 25.072-56 56s25.072 56 56 56 56-25.072 56-56-25.072-56-56-56zM64 384h384V272l-87.515-87.515c-4.686-4.686-12.284-4.686-16.971 0L208 320l-55.515-55.515c-4.686-4.686-12.284-4.686-16.971 0L64 336v48z";
const OFFICIAL_IMAGE_ERROR_PATH: &str = "M464 448H48c-26.51 0-48-21.49-48-48V112c0-26.51 21.49-48 48-48h416c26.51 0 48 21.49 48 48v288c0 26.51-21.49 48-48 48ZM112 120c-30.928 0-56 25.072-56 56s25.072 56 56 56 56-25.072 56-56-25.072-56-56-56ZM64 384h384V272l-87.515-87.515c-4.686-4.686-12.284-4.686-16.971 0L208 320l-55.515-55.515c-4.686-4.686-12.284-4.686-16.971 0L64 336v48Z";
const OFFICIAL_IMAGE_ERROR_BAN_PATH: &str = "M256 8C119.034 8 8 119.033 8 256c0 136.967 111.034 248 248 248s248-111.034 248-248S392.967 8 256 8Zm130.108 117.892c65.448 65.448 70 165.481 20.677 235.637L150.47 105.216c70.204-49.356 170.226-44.735 235.638 20.676ZM125.892 386.108c-65.448-65.448-70-165.481-20.677-235.637L361.53 406.784c-70.203 49.356-170.226 44.736-235.638-20.676Z";

fn svg_path(data: &str) -> Result<tiny_skia::Path, FormatEncodeError> {
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

fn conservative_freedraw_coverage_area(
    path: &super::freedraw::ProjectedPath,
    background_fill: Option<&super::rough::Fill>,
    transform: PngTransform,
    source_stroke_width: f64,
    surface_width: u32,
    surface_height: u32,
) -> u64 {
    let fill_bounds = background_fill.and_then(|fill| fill.path.cubic_bounds());
    let Some((min_x, min_y, max_x, max_y)) =
        super::geometry::union_bounds(path.bounds(), fill_bounds)
    else {
        return 0;
    };
    let points = [
        Point { x: min_x, y: min_y },
        Point { x: max_x, y: min_y },
        Point { x: max_x, y: max_y },
        Point { x: min_x, y: max_y },
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
    let outset = background_fill
        .filter(|fill| matches!(fill.kind, FillKind::Sketch))
        .map_or(0.0, |_| source_stroke_width * transform.scale / 4.0);
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

fn conservative_text_coverage_area(
    text: &super::text::ResolvedText,
    clips: &[PngFrameClip],
    transform: PngTransform,
    surface_width: u32,
    surface_height: u32,
) -> u64 {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut include = |x: f64, y: f64| {
        let point = transform.map_point(Point { x, y });
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    };
    for operation in text.lines.iter().flat_map(|line| &line.path.operations) {
        match *operation {
            super::text::LabelPathOperation::Move { x, y }
            | super::text::LabelPathOperation::Line { x, y } => include(x, y),
            super::text::LabelPathOperation::Quadratic {
                control_x,
                control_y,
                x,
                y,
            } => {
                include(control_x, control_y);
                include(x, y);
            }
            super::text::LabelPathOperation::Cubic {
                control_1_x,
                control_1_y,
                control_2_x,
                control_2_y,
                x,
                y,
            } => {
                include(control_1_x, control_1_y);
                include(control_2_x, control_2_y);
                include(x, y);
            }
            super::text::LabelPathOperation::Close => {}
        }
    }
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return 0;
    }
    for clip in clips {
        let Some((clip_min_x, clip_min_y, clip_max_x, clip_max_y)) = clip.path.cubic_bounds()
        else {
            return 0;
        };
        let corners = [
            Point {
                x: clip_min_x,
                y: clip_min_y,
            },
            Point {
                x: clip_max_x,
                y: clip_min_y,
            },
            Point {
                x: clip_max_x,
                y: clip_max_y,
            },
            Point {
                x: clip_min_x,
                y: clip_max_y,
            },
        ]
        .map(|point| clip.transform.map_point(point));
        let clip_left = corners
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min);
        let clip_top = corners
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min);
        let clip_right = corners
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let clip_bottom = corners
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max);
        min_x = min_x.max(clip_left);
        min_y = min_y.max(clip_top);
        max_x = max_x.min(clip_right);
        max_y = max_y.min(clip_bottom);
    }
    if min_x >= max_x || min_y >= max_y {
        return 0;
    }
    let left = min_x.floor().clamp(0.0, f64::from(surface_width));
    let top = min_y.floor().clamp(0.0, f64::from(surface_height));
    let right = max_x.ceil().clamp(0.0, f64::from(surface_width));
    let bottom = max_y.ceil().clamp(0.0, f64::from(surface_height));
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
    for pixel in rgba.as_chunks_mut::<4>().0 {
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::super::metadata::{
        test_fixture, ArtifactMetadata, ManifestTarget, MetadataSelection,
    };
    use super::super::text::{LabelPath, LabelPathOperation};
    use super::{
        append_chunk, apply_scanline_filter, compress_rgba, label_path, select_scanline_filter,
        ScanlineFilter,
    };

    /// A raster with a flat region, a smooth gradient, a noisy region, and
    /// varying alpha so every scanline filter has something to work on.
    fn synthetic_rgba(width: u32, height: u32) -> Vec<u8> {
        let mut rgba = Vec::new();
        let mut lcg = 0x2545_f491_u32;
        for y in 0..height {
            for x in 0..width {
                lcg = lcg.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = (lcg >> 24) as u8;
                let third = x * 3 / width;
                let pixel = match third {
                    0 => [255, 255, 255, 255],
                    1 => [
                        (x * 255 / width) as u8,
                        (y * 255 / height) as u8,
                        ((x + y) * 127 / (width + height)) as u8,
                        if y % 7 == 0 { 0 } else { 255 },
                    ],
                    _ => [
                        noise,
                        noise.wrapping_mul(3),
                        noise ^ 0x5a,
                        128 | (noise >> 1),
                    ],
                };
                rgba.extend_from_slice(&pixel);
            }
        }
        rgba
    }

    fn mostly_white_rgba(width: u32, height: u32) -> Vec<u8> {
        let mut rgba = vec![255_u8; (width * height * 4) as usize];
        for y in height / 4..height / 2 {
            for x in width / 4..width / 2 {
                let offset = ((y * width + x) * 4) as usize;
                rgba[offset..offset + 3].copy_from_slice(&[0x1e, 0x1e, 0x1e]);
            }
        }
        rgba
    }

    fn encode(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        let Ok(png) = compress_rgba(width, height, rgba).and_then(|stream| stream.assemble(None))
        else {
            panic!("the raster encodes");
        };
        png
    }

    /// Artifact metadata carrying both PNG chunks: the `tEXt` scene wrapper
    /// and the `iTXt` manifest with provenance and the diagnostic summary.
    fn full_metadata() -> ArtifactMetadata {
        let (scene, profile, report) = test_fixture();
        ArtifactMetadata::build(
            &scene,
            profile,
            &report,
            None,
            MetadataSelection {
                provenance: true,
                diagnostic_summary: true,
                embedded_scene: true,
            },
            ManifestTarget::Scene,
        )
        .unwrap_or_else(|_| panic!("metadata with every component builds"))
    }

    /// The `tEXt` and `iTXt` chunks an independent decoder reads after the
    /// image data, as `(chunk type, keyword, text)`.
    fn text_chunks(png: &[u8]) -> Vec<(&'static str, String, String)> {
        let decoder = png::Decoder::new(Cursor::new(png));
        let mut reader = decoder.read_info().expect("PNG structure is valid");
        let mut rgba = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut rgba).expect("PNG pixels decode");
        reader.finish().expect("trailing PNG chunks decode");
        let info = reader.info();
        info.uncompressed_latin1_text
            .iter()
            .map(|chunk| ("tEXt", chunk.keyword.clone(), chunk.text.clone()))
            .chain(info.utf8_text.iter().map(|chunk| {
                (
                    "iTXt",
                    chunk.keyword.clone(),
                    chunk.get_text().expect("iTXt text is UTF-8"),
                )
            }))
            .collect()
    }

    fn decode_rgba(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut decoder = png::Decoder::new(Cursor::new(png));
        decoder.set_transformations(png::Transformations::IDENTITY);
        let mut reader = decoder.read_info().expect("PNG structure is valid");
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
        assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
        let mut rgba = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut rgba).expect("PNG pixels decode");
        rgba.truncate(frame.buffer_size());
        (frame.width, frame.height, rgba)
    }

    fn chunks(png: &[u8]) -> Vec<([u8; 4], &[u8])> {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let mut chunks = Vec::new();
        let mut offset = 8;
        while offset < png.len() {
            let length = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = png[offset + 4..offset + 8].try_into().unwrap();
            chunks.push((kind, &png[offset + 8..offset + 8 + length]));
            offset += 12 + length;
        }
        assert_eq!(offset, png.len());
        chunks
    }

    fn idat(png: &[u8]) -> Vec<u8> {
        chunks(png)
            .into_iter()
            .filter(|(kind, _)| kind == b"IDAT")
            .flat_map(|(_, data)| data.to_vec())
            .collect()
    }

    fn inflate_scanlines(png: &[u8]) -> Vec<u8> {
        let zlib = idat(png);
        let cmf = u16::from(zlib[0]);
        let flg = u16::from(zlib[1]);
        assert_eq!(cmf & 0x0f, 8, "IDAT is a zlib stream using deflate");
        assert_eq!((cmf * 256 + flg) % 31, 0, "zlib header check bits hold");
        miniz_oxide::inflate::decompress_to_vec_zlib(&zlib).expect("IDAT inflates")
    }

    #[test]
    fn encoded_png_round_trips_through_an_independent_decoder() {
        let (width, height) = (37_u32, 23_u32);
        let rgba = synthetic_rgba(width, height);

        let first = encode(width, height, &rgba);
        let second = encode(width, height, &rgba);

        assert_eq!(first, second, "encoding is a pure function of the raster");
        assert_eq!(
            chunks(&first)
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [*b"IHDR", *b"IDAT", *b"IEND"]
        );
        let ihdr = chunks(&first)[0].1;
        assert_eq!(&ihdr[..8], &[0, 0, 0, 37, 0, 0, 0, 23]);
        assert_eq!(
            &ihdr[8..],
            &[8, 6, 0, 0, 0],
            "8-bit RGBA, deflate, adaptive, no interlace"
        );
        assert_eq!(decode_rgba(&first), (width, height, rgba));
    }

    /// Metadata selection assembles up to four candidates and emission one
    /// more from the same raster, so the raster is filtered and deflated once
    /// and every candidate is the same `IDAT` stream followed by its own
    /// metadata chunks (#117).
    #[test]
    fn one_compressed_stream_assembles_every_metadata_candidate() {
        let (width, height) = (37_u32, 23_u32);
        let rgba = synthetic_rgba(width, height);
        let metadata = full_metadata();
        let Ok(stream) = compress_rgba(width, height, &rgba) else {
            panic!("the raster compresses");
        };

        let Ok(bare) = stream.assemble(None) else {
            panic!("the baseline candidate assembles");
        };
        let Ok(embedded) = stream.assemble(Some(&metadata)) else {
            panic!("the embedded candidate assembles");
        };
        let Ok(emitted) = stream.assemble(None) else {
            panic!("the stream survives assembly for the emission pass");
        };

        assert_eq!(
            bare, emitted,
            "assembly neither consumes nor alters the stream"
        );
        assert_eq!(
            chunks(&bare)
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [*b"IHDR", *b"IDAT", *b"IEND"]
        );
        assert_eq!(
            chunks(&embedded)
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>(),
            [*b"IHDR", *b"IDAT", *b"tEXt", *b"iTXt", *b"IEND"]
        );
        assert_eq!(
            idat(&bare),
            idat(&embedded),
            "the IDAT stream does not depend on the metadata chunks"
        );
        assert_eq!(decode_rgba(&bare), (width, height, rgba.clone()));
        assert_eq!(decode_rgba(&embedded), (width, height, rgba));

        let wrapper = metadata
            .scene_wrapper()
            .expect("the fixture embeds a scene")
            .iter()
            .map(|byte| char::from(*byte))
            .collect::<String>();
        let manifest = metadata
            .manifest()
            .expect("the fixture carries a manifest")
            .to_owned();
        assert_eq!(text_chunks(&bare), []);
        assert_eq!(
            text_chunks(&embedded),
            [
                (
                    "tEXt",
                    "application/vnd.excalidraw+json".to_owned(),
                    wrapper
                ),
                (
                    "iTXt",
                    "excalidraw-render:artifact-manifest".to_owned(),
                    manifest
                ),
            ]
        );
    }

    #[test]
    fn idat_is_a_deflate_compressed_stream_of_filtered_scanlines() {
        let (width, height) = (37_u32, 23_u32);
        let rgba = synthetic_rgba(width, height);
        let png = encode(width, height, &rgba);

        let row_bytes = width as usize * 4 + 1;
        let scanlines = inflate_scanlines(&png);
        assert_eq!(scanlines.len(), row_bytes * height as usize);
        let filter_types = scanlines
            .chunks_exact(row_bytes)
            .map(|row| row[0])
            .collect::<Vec<_>>();
        assert!(filter_types.iter().all(|kind| *kind <= 4));
        assert!(
            filter_types.iter().any(|kind| *kind != 0),
            "noisy scanlines pick a predictive filter: {filter_types:?}"
        );
        assert!(
            idat(&png).len() < scanlines.len(),
            "the stream is smaller than the scanlines it carries"
        );
    }

    #[test]
    fn flat_rasters_compress_far_below_their_stored_size() {
        let (width, height) = (512_u32, 256_u32);
        let rgba = mostly_white_rgba(width, height);
        let raw_bytes = rgba.len() + height as usize;

        let png = encode(width, height, &rgba);

        assert!(
            png.len() * 20 < raw_bytes,
            "{} PNG bytes is not far below {raw_bytes} stored bytes (#113)",
            png.len()
        );
        assert_eq!(decode_rgba(&png), (width, height, rgba));
        let row_bytes = width as usize * 4 + 1;
        assert!(
            inflate_scanlines(&png)
                .chunks_exact(row_bytes)
                .all(|row| row[0] == 0),
            "flat vector-style scanlines stay unfiltered, which deflates best"
        );
    }

    #[test]
    fn scanline_filter_selection_keeps_flat_rows_raw_and_predicts_noisy_rows() {
        let zeros = vec![0_u8; 60 * 4];
        let flat = [200_u8, 100, 50, 255].repeat(60);
        let gentle_gradient = (0..60_u8).flat_map(|x| [x, x, x, 255]).collect::<Vec<_>>();
        let steep_gradient = (0..60_u8)
            .flat_map(|x| [x * 3, x * 3, x * 3, 255])
            .collect::<Vec<_>>();
        let steep_gradient_plus_one = (0..60_u8)
            .flat_map(|x| [x * 3 + 1, x * 3 + 1, x * 3 + 1, 255])
            .collect::<Vec<_>>();

        // Rows whose bytes mostly repeat their left or upper neighbour are
        // vector-style content; unfiltered bytes deflate best there.
        assert_eq!(select_scanline_filter(&flat, &zeros), ScanlineFilter::None);
        assert_eq!(
            select_scanline_filter(&steep_gradient, &steep_gradient),
            ScanlineFilter::None
        );
        // Otherwise the smallest sum of absolute residuals wins, with ties
        // resolved toward the lowest filter type.
        assert_eq!(
            select_scanline_filter(&gentle_gradient, &zeros),
            ScanlineFilter::Sub
        );
        assert_eq!(
            select_scanline_filter(&steep_gradient_plus_one, &steep_gradient),
            ScanlineFilter::Up
        );
    }

    #[test]
    fn every_scanline_filter_inverts_under_an_independent_decoder() {
        let (width, height) = (11_u32, 5_u32);
        let rgba = synthetic_rgba(width, height);
        let pixel_row_bytes = width as usize * 4;
        let zeros = vec![0_u8; pixel_row_bytes];

        let filters = [
            ScanlineFilter::None,
            ScanlineFilter::Sub,
            ScanlineFilter::Up,
            ScanlineFilter::Average,
            ScanlineFilter::Paeth,
        ];
        let mut scanlines = Vec::new();
        for (row_index, row) in rgba.chunks_exact(pixel_row_bytes).enumerate() {
            let previous = if row_index == 0 {
                &zeros[..]
            } else {
                &rgba[(row_index - 1) * pixel_row_bytes..row_index * pixel_row_bytes]
            };
            let filter = filters[row_index % filters.len()];
            let mut residuals = vec![0_u8; pixel_row_bytes];
            apply_scanline_filter(filter, row, previous, &mut residuals);
            scanlines.push(filter.type_byte());
            scanlines.extend_from_slice(&residuals);
        }

        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = [0_u8; 13];
        ihdr[..4].copy_from_slice(&width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&height.to_be_bytes());
        ihdr[8] = 8;
        ihdr[9] = 6;
        assert!(append_chunk(&mut png, *b"IHDR", &ihdr).is_ok());
        let zlib = miniz_oxide::deflate::compress_to_vec_zlib(&scanlines, 6);
        assert!(append_chunk(&mut png, *b"IDAT", &zlib).is_ok());
        assert!(append_chunk(&mut png, *b"IEND", &[]).is_ok());

        assert_eq!(decode_rgba(&png), (width, height, rgba));
    }

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
