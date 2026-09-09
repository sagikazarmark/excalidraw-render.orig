//! The direct SVG backend (stage 5 and 6): `SvgPlan` lowers the resolved scene
//! into `SvgElement`s in the units SVG emits, `measure` and `encode_exact` run
//! the writer twice through a counting and a real `SvgSink` so the artifact is
//! sized before it is allocated, and the plan carries the font bundle, the
//! artifact metadata and the mandatory notices the writer embeds. `sink` is
//! that writer abstraction; `write` spells the document.

mod sink;
mod write;

pub(super) use sink::SvgSink;
pub(super) use write::freedraw_path_data;
#[cfg(test)]
pub(super) use write::push_rough_path;

use std::sync::Arc;

use crate::normalization::NormalizedElementSource;
use crate::{color::CanonicalColor, RenderRequest, Sizing};

use super::metadata::ArtifactMetadata;
use super::notices::MandatoryNotices;
use super::resolve::{
    DarkModeImageFilter, FallbackColors, FallbackSource, ResolvedClip, ResolvedElement,
    ResolvedImageAsset, ResolvedImageProjection, ResolvedLabelMask, ResolvedScene,
    ResolvedTransform, StrokeStyle,
};
use super::rough::{Drawing, Fill};
use super::svg_fonts::Bundle;
use super::text::{ResolvedLabel, ResolvedText};
use super::FormatEncodeError;

use sink::CountingWriter;

/// A transform in the units SVG emits: degrees of rotation, logical pixels for
/// translation and centre. `ResolvedTransform` carries radians, so the
/// conversion happens once here rather than at each element variant — the same
/// coordinate-space boundary `PngTransform` marks for device pixels.
#[derive(Clone, Copy, Debug)]
struct SvgTransform {
    translation_x: f64,
    translation_y: f64,
    rotation_degrees: f64,
    center_x: f64,
    center_y: f64,
}

impl SvgTransform {
    fn new(transform: &ResolvedTransform) -> Self {
        Self {
            translation_x: transform.translation_x,
            translation_y: transform.translation_y,
            rotation_degrees: 180.0 * transform.angle_radians / std::f64::consts::PI,
            center_x: transform.center_x,
            center_y: transform.center_y,
        }
    }
}

#[derive(Debug)]
pub(super) struct SvgPlan {
    logical_width: f64,
    logical_height: f64,
    output_width: f64,
    output_height: f64,
    background: Option<CanonicalColor>,
    fallback_colors: FallbackColors,
    font_bundle: Bundle,
    mandatory_notices: Option<MandatoryNotices>,
    metadata: Option<ArtifactMetadata>,
    elements: Vec<SvgElement>,
}

/// The `<defs>` filter standing in for the reference filters a dark-mode image
/// passes through ([`DarkModeImageFilter`]): the renderer lowers the root
/// `THEME_FILTER` into its vector colors and carries no root filter, so each
/// image references the whole chain it needs. The definitions spell the CSS
/// filter functions as the Filter Effects primitives they are defined as, in
/// sRGB like the functions, so a viewer without filter-function support still
/// applies them.
trait SvgImageFilterDefinition {
    fn id(self) -> &'static str;

    /// The definition, primitive by primitive in filter-function order.
    fn definition(self) -> &'static str;
}

impl SvgImageFilterDefinition for DarkModeImageFilter {
    fn id(self) -> &'static str {
        match self {
            Self::RasterImage => "dark-mode-raster-image",
            Self::NestedSvgImage => "dark-mode-nested-svg-image",
        }
    }

    fn definition(self) -> &'static str {
        match self {
            Self::RasterImage => concat!(
                "<filter id=\"dark-mode-raster-image\" color-interpolation-filters=\"sRGB\">",
                // invert(100%)
                "<feComponentTransfer>",
                "<feFuncR type=\"table\" tableValues=\"1 0\"/>",
                "<feFuncG type=\"table\" tableValues=\"1 0\"/>",
                "<feFuncB type=\"table\" tableValues=\"1 0\"/>",
                "</feComponentTransfer>",
                // hue-rotate(180deg) saturate(1.25)
                "<feColorMatrix type=\"hueRotate\" values=\"180\"/>",
                "<feColorMatrix type=\"saturate\" values=\"1.25\"/>",
                // invert(93%) hue-rotate(180deg)
                "<feComponentTransfer>",
                "<feFuncR type=\"table\" tableValues=\"0.93 0.07\"/>",
                "<feFuncG type=\"table\" tableValues=\"0.93 0.07\"/>",
                "<feFuncB type=\"table\" tableValues=\"0.93 0.07\"/>",
                "</feComponentTransfer>",
                "<feColorMatrix type=\"hueRotate\" values=\"180\"/>",
                "</filter>",
            ),
            Self::NestedSvgImage => concat!(
                "<filter id=\"dark-mode-nested-svg-image\" color-interpolation-filters=\"sRGB\">",
                // invert(93%) hue-rotate(180deg)
                "<feComponentTransfer>",
                "<feFuncR type=\"table\" tableValues=\"0.93 0.07\"/>",
                "<feFuncG type=\"table\" tableValues=\"0.93 0.07\"/>",
                "<feFuncB type=\"table\" tableValues=\"0.93 0.07\"/>",
                "</feComponentTransfer>",
                "<feColorMatrix type=\"hueRotate\" values=\"180\"/>",
                "</filter>",
            ),
        }
    }
}

#[derive(Debug)]
enum SvgElement {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        frame_clips: Vec<ResolvedClip>,
        transform: SvgTransform,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
        background_color: CanonicalColor,
        label_mask: Option<ResolvedLabelMask>,
        link: Option<String>,
        generated_label: Option<ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
        width: f64,
        height: f64,
    },
    Freedraw {
        source: NormalizedElementSource,
        path_data: String,
        background_fill: Option<Fill>,
        frame_clips: Vec<ResolvedClip>,
        transform: SvgTransform,
        color: CanonicalColor,
        background_color: Option<CanonicalColor>,
        source_stroke_width: f64,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    /// A frame's outline and name. The frame carries no opacity here: the
    /// reference SVG's frame `<rect>` has no opacity attribute, while its
    /// canvas sets `globalAlpha` from the element's render opacity, which PNG
    /// and PDF follow (format-specific parity, spec 8.1). The frame's opacity
    /// still multiplies its children's, resolved before the plan.
    Frame {
        source: NormalizedElementSource,
        name: Option<SvgFrameName>,
        /// The outline's stroke when the frame paints one: `FRAME_STYLE.strokeColor`
        /// for the color mode, as the reference SVG strokes every frame kind.
        outline_stroke_color: Option<CanonicalColor>,
        frame_clips: Vec<ResolvedClip>,
        transform: SvgTransform,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Text {
        source: NormalizedElementSource,
        text: ResolvedText,
        frame_clips: Vec<ResolvedClip>,
        transform: ResolvedTransform,
        color: CanonicalColor,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Image {
        source: NormalizedElementSource,
        asset: Arc<ResolvedImageAsset>,
        /// The dark-mode filter the `<image>` references, defined in `<defs>`.
        dark_mode_filter: Option<DarkModeImageFilter>,
        frame_clips: Vec<ResolvedClip>,
        /// The image's rounded-corner clip radius when it has a roundness
        /// record; the SVG defines the clip as a `<rect rx ry>`.
        corner_radius: Option<f64>,
        transform: ResolvedTransform,
        opacity: f64,
        /// The reference export's rounded viewport and content placement.
        projection: ResolvedImageProjection,
        scale: [i8; 2],
        link: Option<String>,
        /// The element's unrounded size: the flip axis and the link rectangle.
        width: f64,
        height: f64,
    },
    Fallback {
        source: FallbackSource,
        clip_index: usize,
        label: Option<String>,
        link: Option<String>,
        frame_clips: Vec<ResolvedClip>,
        width: f64,
        height: f64,
        transform: SvgTransform,
        opacity: f64,
    },
}

impl SvgElement {
    fn frame_clips(&self) -> &[ResolvedClip] {
        match self {
            Self::RoughOutline { frame_clips, .. }
            | Self::Freedraw { frame_clips, .. }
            | Self::Frame { frame_clips, .. }
            | Self::Text { frame_clips, .. }
            | Self::Image { frame_clips, .. }
            | Self::Fallback { frame_clips, .. } => frame_clips,
        }
    }
}

/// A frame's name as its own unrotated, unclipped, fully opaque text element
/// above the frame, the way the reference export synthesizes it.
#[derive(Debug)]
struct SvgFrameName {
    label: ResolvedLabel,
    transform: SvgTransform,
    color: CanonicalColor,
}

impl SvgPlan {
    pub(super) fn new(
        scene: ResolvedScene,
        request: &RenderRequest,
        mut font_bundle: Bundle,
    ) -> Result<Self, FormatEncodeError> {
        let scale = match request.sizing() {
            Sizing::Scale(scale) => scale,
            Sizing::MaxDimension(_) => 1.0,
        };
        let mandatory_notices = font_bundle.take_mandatory_notices();
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(scene.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "svg_elements",
            })?;
        let mut fallback_index = 0_usize;
        for element in scene.elements {
            match element {
                ResolvedElement::Outline(outline) => {
                    if outline.source.id().is_empty() {
                        return Err(FormatEncodeError::Encoding(format!(
                            "outline at {} has no normalized identity",
                            outline.source.pointer()
                        )));
                    }
                    let frame_clips = if outline.generated_label.is_some() {
                        Vec::new()
                    } else {
                        outline.frame_clips
                    };
                    elements.push(SvgElement::RoughOutline {
                        source: outline.source,
                        drawing: outline.drawing,
                        frame_clips,
                        transform: SvgTransform::new(&outline.transform),
                        stroke_color: outline.stroke_color,
                        fill_color: outline.fill_color,
                        stroke_width: outline.stroke_width,
                        stroke_style: outline.stroke_style,
                        source_stroke_width: outline.source_stroke_width,
                        opacity: if outline.generated_label.is_some() {
                            outline.element_opacity
                        } else {
                            outline.opacity
                        },
                        background_color: outline.background_color,
                        label_mask: outline.label_mask,
                        link: outline.link,
                        generated_label: outline.generated_label,
                        generated_label_color: outline.generated_label_color,
                        generated_label_offset_y: outline.generated_label_offset_y,
                        width: outline.width,
                        height: outline.height,
                    });
                }
                ResolvedElement::Freedraw(freedraw) => {
                    elements.push(SvgElement::Freedraw {
                        source: freedraw.source,
                        path_data: freedraw_path_data(&freedraw.path)?,
                        background_fill: freedraw.background_fill,
                        frame_clips: freedraw.frame_clips,
                        transform: SvgTransform::new(&freedraw.transform),
                        color: freedraw.color,
                        background_color: freedraw.background_color,
                        source_stroke_width: freedraw.source_stroke_width,
                        opacity: freedraw.opacity,
                        link: freedraw.link,
                        width: freedraw.width,
                        height: freedraw.height,
                    });
                }
                ResolvedElement::Frame(frame) => {
                    elements.push(SvgElement::Frame {
                        source: frame.source,
                        name: frame.name.map(|name| SvgFrameName {
                            transform: SvgTransform::new(&name.transform),
                            color: name.color,
                            label: name.label,
                        }),
                        outline_stroke_color: frame.outline.map(|outline| outline.stroke_color),
                        frame_clips: frame.frame_clips,
                        transform: SvgTransform::new(&frame.transform),
                        link: frame.link,
                        width: frame.width,
                        height: frame.height,
                    });
                    fallback_index = fallback_index.saturating_add(1);
                }
                ResolvedElement::Text(text) => {
                    elements.push(SvgElement::Text {
                        source: text.source,
                        text: text.text,
                        frame_clips: text.frame_clips,
                        transform: text.transform,
                        color: text.color,
                        opacity: text.opacity,
                        link: text.link,
                        width: text.width,
                        height: text.height,
                    });
                }
                ResolvedElement::Image(image) => {
                    elements.push(SvgElement::Image {
                        source: image.source,
                        asset: image.asset,
                        dark_mode_filter: image.dark_mode_filter,
                        frame_clips: image.frame_clips,
                        corner_radius: image.corner_clip.map(|clip| clip.radius),
                        transform: image.transform,
                        opacity: image.opacity,
                        projection: image.projection,
                        scale: image.scale,
                        link: image.link,
                        width: image.width,
                        height: image.height,
                    });
                }
                ResolvedElement::ImageFallback(_) => {}
                ResolvedElement::Fallback(fallback) => {
                    elements.push(SvgElement::Fallback {
                        source: fallback.source,
                        clip_index: fallback_index,
                        label: fallback.label.map(|label| label.display_text),
                        link: fallback.link,
                        frame_clips: fallback.frame_clips,
                        width: fallback.width,
                        height: fallback.height,
                        transform: SvgTransform::new(&fallback.transform),
                        opacity: fallback.opacity,
                    });
                    fallback_index = fallback_index.saturating_add(1);
                }
            }
        }
        Ok(Self {
            logical_width: scene.logical_width,
            logical_height: scene.logical_height,
            output_width: scene.logical_width * scale,
            output_height: scene.logical_height * scale,
            background: scene.include_background.then_some(scene.background_color),
            fallback_colors: scene.fallback_colors,
            font_bundle,
            mandatory_notices,
            metadata: None,
            elements,
        })
    }

    /// The dark-mode image filters the plan references, each once, raster
    /// before nested-SVG.
    fn image_filters(&self) -> impl Iterator<Item = DarkModeImageFilter> + '_ {
        [
            DarkModeImageFilter::RasterImage,
            DarkModeImageFilter::NestedSvgImage,
        ]
        .into_iter()
        .filter(move |filter| {
            self.elements.iter().any(|element| {
                matches!(
                    element,
                    SvgElement::Image { dark_mode_filter: Some(referenced), .. } if referenced == filter
                )
            })
        })
    }

    pub(super) fn measure(&self) -> Result<usize, FormatEncodeError> {
        let mut output = CountingWriter::default();
        self.write(&mut output)?;
        if output.overflowed {
            Err(svg_size_error())
        } else {
            Ok(output.len)
        }
    }

    pub(super) fn encode_exact(&self, measured: usize) -> Result<Vec<u8>, FormatEncodeError> {
        let mut svg = String::new();
        svg.try_reserve_exact(measured)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "artifact",
                subject: "svg_output",
            })?;
        self.write(&mut svg)?;
        if svg.len() != measured {
            return Err(FormatEncodeError::Encoding(
                "measured and encoded SVG lengths differ".to_owned(),
            ));
        }
        Ok(svg.into_bytes())
    }

    pub(super) fn set_metadata(&mut self, metadata: ArtifactMetadata) {
        self.metadata = Some(metadata);
        self.mandatory_notices = None;
    }

    pub(super) fn take_mandatory_notices(&mut self) -> Option<MandatoryNotices> {
        self.mandatory_notices.take()
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
        self.elements.get(index).map(|element| match element {
            SvgElement::RoughOutline { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Freedraw { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Frame { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Text { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Image { source, .. } => (Some(source.id()), source.pointer()),
            SvgElement::Fallback { source, .. } => (source.id.as_deref(), source.pointer.as_str()),
        })
    }
}

impl SvgElement {
    fn link(&self) -> Option<&str> {
        match self {
            Self::RoughOutline { link, .. }
            | Self::Freedraw { link, .. }
            | Self::Frame { link, .. }
            | Self::Text { link, .. }
            | Self::Image { link, .. }
            | Self::Fallback { link, .. } => link.as_deref(),
        }
    }
}

fn svg_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("SVG plan exceeds serializer capacity".to_owned())
}
