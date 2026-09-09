use crate::normalization::NormalizedElementSource;
use crate::{color::CanonicalColor, RenderRequest, Sizing};

use super::resolve::{DarkModeImageFilter, ResolvedElement, ResolvedScene, StrokeStyle};
use super::rough::{Drawing, FillKind, Path, PathOperation};
use super::FormatEncodeError;

pub(super) trait SvgSink: std::fmt::Write {
    fn push_str(&mut self, value: &str);

    fn push(&mut self, value: char);
}

impl SvgSink for String {
    fn push_str(&mut self, value: &str) {
        String::push_str(self, value);
    }

    fn push(&mut self, value: char) {
        String::push(self, value);
    }
}

impl<T: SvgSink + ?Sized> SvgSink for &mut T {
    fn push_str(&mut self, value: &str) {
        T::push_str(self, value);
    }

    fn push(&mut self, value: char) {
        T::push(self, value);
    }
}

#[derive(Default)]
struct CountingWriter {
    len: usize,
    overflowed: bool,
}

impl CountingWriter {
    fn add(&mut self, bytes: usize) {
        if let Some(len) = self.len.checked_add(bytes) {
            self.len = len;
        } else {
            self.overflowed = true;
        }
    }
}

impl std::fmt::Write for CountingWriter {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.add(value.len());
        Ok(())
    }
}

impl SvgSink for CountingWriter {
    fn push_str(&mut self, value: &str) {
        self.add(value.len());
    }

    fn push(&mut self, value: char) {
        self.add(value.len_utf8());
    }
}

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
    fn new(transform: &super::resolve::ResolvedTransform) -> Self {
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
    fallback_colors: super::resolve::FallbackColors,
    font_bundle: super::svg_fonts::Bundle,
    mandatory_notices: Option<super::notices::MandatoryNotices>,
    metadata: Option<super::metadata::ArtifactMetadata>,
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
        frame_clips: Vec<super::resolve::ResolvedClip>,
        transform: SvgTransform,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
        background_color: CanonicalColor,
        label_mask: Option<super::resolve::ResolvedLabelMask>,
        link: Option<String>,
        generated_label: Option<super::text::ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
        width: f64,
        height: f64,
    },
    Freedraw {
        source: NormalizedElementSource,
        path_data: String,
        background_fill: Option<super::rough::Fill>,
        frame_clips: Vec<super::resolve::ResolvedClip>,
        transform: SvgTransform,
        color: CanonicalColor,
        background_color: Option<CanonicalColor>,
        source_stroke_width: f64,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Frame {
        source: NormalizedElementSource,
        name: Option<SvgFrameName>,
        /// The outline's stroke when the frame paints one: `FRAME_STYLE.strokeColor`
        /// for the color mode, as the reference SVG strokes every frame kind.
        outline_stroke_color: Option<CanonicalColor>,
        frame_clips: Vec<super::resolve::ResolvedClip>,
        transform: SvgTransform,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Text {
        source: NormalizedElementSource,
        text: super::text::ResolvedText,
        frame_clips: Vec<super::resolve::ResolvedClip>,
        transform: super::resolve::ResolvedTransform,
        color: CanonicalColor,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Image {
        source: NormalizedElementSource,
        asset: std::sync::Arc<super::resolve::ResolvedImageAsset>,
        /// The dark-mode filter the `<image>` references, defined in `<defs>`.
        dark_mode_filter: Option<DarkModeImageFilter>,
        frame_clips: Vec<super::resolve::ResolvedClip>,
        /// The image's rounded-corner clip radius when it has a roundness
        /// record; the SVG defines the clip as a `<rect rx ry>`.
        corner_radius: Option<f64>,
        transform: super::resolve::ResolvedTransform,
        opacity: f64,
        /// The reference export's rounded viewport and content placement.
        projection: super::resolve::ResolvedImageProjection,
        scale: [i8; 2],
        link: Option<String>,
        /// The element's unrounded size: the flip axis and the link rectangle.
        width: f64,
        height: f64,
    },
    Fallback {
        source: super::resolve::FallbackSource,
        clip_index: usize,
        label: Option<String>,
        link: Option<String>,
        frame_clips: Vec<super::resolve::ResolvedClip>,
        width: f64,
        height: f64,
        transform: SvgTransform,
        opacity: f64,
    },
}

impl SvgElement {
    fn frame_clips(&self) -> &[super::resolve::ResolvedClip] {
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
    label: super::text::ResolvedLabel,
    transform: SvgTransform,
    color: CanonicalColor,
}

/// Every frame clip the plan references, in first-use order, each defined once
/// in `<defs>` and referenced by every child it clips.
fn frame_clip_definitions(
    elements: &[SvgElement],
) -> Result<Vec<&super::resolve::ResolvedClip>, FormatEncodeError> {
    let mut definitions = Vec::<&super::resolve::ResolvedClip>::new();
    for clip in elements.iter().flat_map(SvgElement::frame_clips) {
        if !definitions
            .iter()
            .any(|known| known.frame_source_index == clip.frame_source_index)
        {
            definitions
                .try_reserve(1)
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_plan",
                    subject: "svg_frame_clip_definitions",
                })?;
            definitions.push(clip);
        }
    }
    Ok(definitions)
}

fn push_frame_clip_definition<W: SvgSink + ?Sized>(
    svg: &mut W,
    clip: &super::resolve::ResolvedClip,
) -> Result<(), FormatEncodeError> {
    svg.push_str("<clipPath id=\"");
    push_frame_clip_id(svg, clip)?;
    svg.push_str("\"><rect transform=\"translate(");
    push_svg_number(
        svg,
        clip.transform.translation_x,
        "SVG frame clip translation x",
    )?;
    svg.push(' ');
    push_svg_number(
        svg,
        clip.transform.translation_y,
        "SVG frame clip translation y",
    )?;
    svg.push_str(") rotate(");
    push_svg_number(
        svg,
        180.0 * clip.transform.angle_radians / std::f64::consts::PI,
        "SVG frame clip rotation",
    )?;
    svg.push(' ');
    push_svg_number(svg, clip.transform.center_x, "SVG frame clip center x")?;
    svg.push(' ');
    push_svg_number(svg, clip.transform.center_y, "SVG frame clip center y")?;
    svg.push_str(")\" width=\"");
    push_svg_number(svg, clip.width, "SVG frame clip width")?;
    svg.push_str("\" height=\"");
    push_svg_number(svg, clip.height, "SVG frame clip height")?;
    svg.push('"');
    push_svg_corner_radius(svg, clip.radius)?;
    svg.push_str("/></clipPath>");
    Ok(())
}

/// `rx`/`ry` for a rounded clip rectangle: a frame's in a scene export, or an
/// image's corner clip. The rectangular frame-export clip has none, as in the
/// reference.
fn push_svg_corner_radius<W: SvgSink + ?Sized>(
    svg: &mut W,
    radius: f64,
) -> Result<(), FormatEncodeError> {
    if radius > 0.0 {
        svg.push_str(" rx=\"");
        push_svg_number(svg, radius, "SVG clip corner radius")?;
        svg.push_str("\" ry=\"");
        push_svg_number(svg, radius, "SVG clip corner radius")?;
        svg.push('"');
    }
    Ok(())
}

fn push_frame_clip_prefix<W: SvgSink + ?Sized>(
    svg: &mut W,
    clips: &[super::resolve::ResolvedClip],
) -> Result<(), FormatEncodeError> {
    for clip in clips {
        svg.push_str("<g clip-path=\"url(#");
        push_frame_clip_id(svg, clip)?;
        svg.push_str(")\">");
    }
    Ok(())
}

/// The frame name as the reference's synthesized text element: an unrotated
/// group at the label box's top-left corner holding one left-aligned 14 px
/// line on Helvetica's metadata baseline, in the theme's frame-name color.
fn push_frame_name<W: SvgSink + ?Sized>(
    svg: &mut W,
    source: &NormalizedElementSource,
    name: &SvgFrameName,
) -> Result<(), FormatEncodeError> {
    let SvgFrameName {
        label,
        transform,
        color,
    } = name;
    svg.push_str("<g transform=\"translate(");
    push_svg_number(svg, transform.translation_x, "SVG frame name translation x")?;
    svg.push(' ');
    push_svg_number(svg, transform.translation_y, "SVG frame name translation y")?;
    svg.push_str(") rotate(");
    push_svg_number(svg, transform.rotation_degrees, "SVG frame name rotation")?;
    svg.push(' ');
    push_svg_number(svg, transform.center_x, "SVG frame name center x")?;
    svg.push(' ');
    push_svg_number(svg, transform.center_y, "SVG frame name center y")?;
    svg.push_str(")\"><text data-frame-name=\"");
    push_escaped_attribute(svg, source.id());
    svg.push_str("\" data-source-text=\"");
    push_escaped_attribute(svg, &label.text);
    svg.push_str("\" x=\"");
    if label.glyphs.is_empty() {
        svg.push('0');
    } else {
        for (index, glyph) in label.glyphs.iter().enumerate() {
            if index != 0 {
                svg.push(' ');
            }
            push_svg_number(svg, glyph.x, "SVG frame name glyph x")?;
        }
    }
    svg.push_str("\" y=\"");
    if label.glyphs.is_empty() {
        push_svg_number(svg, label.baseline, "SVG frame name baseline")?;
    } else {
        for (index, glyph) in label.glyphs.iter().enumerate() {
            if index != 0 {
                svg.push(' ');
            }
            push_svg_number(svg, glyph.y, "SVG frame name glyph y")?;
        }
    }
    svg.push_str("\" font-family=\"");
    push_escaped_attribute(
        svg,
        super::svg_fonts::Bundle::effective_family("Liberation Sans"),
    );
    svg.push_str("\" font-size=\"");
    push_svg_number(
        svg,
        super::text::FRAME_NAME_FONT_SIZE,
        "SVG frame name font size",
    )?;
    svg.push_str("\" fill=\"");
    color.write_hex(svg).map_err(|_| {
        FormatEncodeError::Encoding("failed to write SVG frame name color".to_owned())
    })?;
    svg.push_str("\" text-anchor=\"start\">");
    push_escaped_text(svg, &label.display_text);
    svg.push_str("</text></g>");
    Ok(())
}

fn push_svg_link_hit_rect<W: SvgSink + ?Sized>(
    svg: &mut W,
    link: Option<&str>,
    width: f64,
    height: f64,
) -> Result<(), FormatEncodeError> {
    if link.is_none() {
        return Ok(());
    }
    svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
    push_svg_number(svg, width, "SVG link hit width")?;
    svg.push_str("\" height=\"");
    push_svg_number(svg, height, "SVG link hit height")?;
    svg.push_str("\" fill=\"transparent\"/>");
    Ok(())
}

/// Writes one glyph run's ` x="..." y="..."` with one value per character of
/// the run's `<tspan>` content. SVG assigns the nth value to the nth character
/// of the content, which is in logical order, while the line's glyphs sit in
/// visual order; the run's logical glyph mapping bridges the two, so a
/// right-to-left run lists its positions from its rightmost glyph down to its
/// leftmost. `offset_y` shifts every `y`; `subject` names the run kind in the
/// error for a mapping outside the line's glyphs.
fn push_svg_run_positions<W: SvgSink + ?Sized>(
    svg: &mut W,
    line: &super::text::ResolvedLabel,
    run: &super::text::ResolvedTextRun,
    offset_y: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    let glyph = |index: &usize| {
        line.glyphs.get(*index).ok_or_else(|| {
            FormatEncodeError::Encoding(format!("{subject} has invalid glyph bounds"))
        })
    };
    svg.push_str(" x=\"");
    for (position, index) in run.logical_glyphs.iter().enumerate() {
        if position != 0 {
            svg.push(' ');
        }
        push_svg_number(svg, glyph(index)?.x, "SVG run glyph x")?;
    }
    svg.push_str("\" y=\"");
    for (position, index) in run.logical_glyphs.iter().enumerate() {
        if position != 0 {
            svg.push(' ');
        }
        push_svg_number(svg, glyph(index)?.y + offset_y, "SVG run glyph y")?;
    }
    svg.push('"');
    Ok(())
}

/// The clip's identity is its frame's, so every child of one frame references
/// the one definition. The source index is renderer-owned: an element id is
/// untrusted input and need not be a usable XML ID.
fn push_frame_clip_id<W: SvgSink + ?Sized>(
    svg: &mut W,
    clip: &super::resolve::ResolvedClip,
) -> Result<(), FormatEncodeError> {
    write!(svg, "frame-clip-{}", clip.frame_source_index)
        .map_err(|_| FormatEncodeError::Encoding("failed to write SVG frame clip ID".to_owned()))
}

/// An image's rounded-corner clip, keyed like a frame clip by the image's
/// source index rather than its untrusted element id. The reference defines
/// the clip as a `<clipPath>` holding a `<rect>` of the element's size with
/// `getCornerRadius(min(width, height))` corners and references it from the
/// image's translated and rotated group, so the clip sits in element space,
/// outside the crop viewport and the flip. The plan defines it in `<defs>`
/// beside the frame clips, in paint order.
fn push_image_clip_id<W: SvgSink + ?Sized>(
    svg: &mut W,
    source: &NormalizedElementSource,
) -> Result<(), FormatEncodeError> {
    write!(svg, "image-clip-{}", source.source_index())
        .map_err(|_| FormatEncodeError::Encoding("failed to write SVG image clip ID".to_owned()))
}

fn push_image_clip_definition<W: SvgSink + ?Sized>(
    svg: &mut W,
    source: &NormalizedElementSource,
    width: f64,
    height: f64,
    radius: f64,
) -> Result<(), FormatEncodeError> {
    svg.push_str("<clipPath id=\"");
    push_image_clip_id(svg, source)?;
    svg.push_str("\"><rect width=\"");
    push_svg_number(svg, width, "SVG image clip width")?;
    svg.push_str("\" height=\"");
    push_svg_number(svg, height, "SVG image clip height")?;
    svg.push('"');
    push_svg_corner_radius(svg, radius)?;
    svg.push_str("/></clipPath>");
    Ok(())
}

impl SvgPlan {
    pub(super) fn new(
        scene: ResolvedScene,
        request: &RenderRequest,
        mut font_bundle: super::svg_fonts::Bundle,
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
                        opacity: frame.opacity,
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

    fn write<W: SvgSink + ?Sized>(&self, mut svg: &mut W) -> Result<(), FormatEncodeError> {
        svg.push_str("<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 ");
        push_svg_number(svg, self.logical_width, "SVG width")?;
        svg.push(' ');
        push_svg_number(svg, self.logical_height, "SVG height")?;
        svg.push_str("\" width=\"");
        push_svg_number(svg, self.output_width, "scaled SVG width")?;
        svg.push_str("\" height=\"");
        push_svg_number(svg, self.output_height, "scaled SVG height")?;
        svg.push_str("\">");
        let scene_wrapper = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::scene_wrapper);
        let manifest = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::manifest);
        svg.push_str("<!-- svg-source:excalidraw --><metadata>");
        if let Some(wrapper) = scene_wrapper {
            svg.push_str(concat!(
                "<!-- payload-type:application/vnd.excalidraw+json -->",
                "<!-- payload-version:2 --><!-- payload-start -->"
            ));
            push_base64(svg, wrapper)?;
            svg.push_str("<!-- payload-end -->");
        }
        if let Some(manifest) = manifest {
            svg.push_str("<excalidraw-render:manifest xmlns:excalidraw-render=\"urn:excalidraw-render:artifact-manifest:1\">");
            push_escaped_text(svg, manifest);
            svg.push_str("</excalidraw-render:manifest>");
        }
        svg.push_str("</metadata><defs>");
        for clip in frame_clip_definitions(&self.elements)? {
            push_frame_clip_definition(&mut svg, clip)?;
        }
        for element in &self.elements {
            if let SvgElement::Image {
                source,
                corner_radius: Some(radius),
                width,
                height,
                ..
            } = element
            {
                push_image_clip_definition(&mut svg, source, *width, *height, *radius)?;
            }
        }
        for filter in self.image_filters() {
            svg.push_str(filter.definition());
        }
        self.font_bundle.write_style(svg, push_base64)?;
        svg.push_str("</defs>");
        if let Some(background) = &self.background {
            svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
            push_svg_number(&mut svg, self.logical_width, "SVG background width")?;
            svg.push_str("\" height=\"");
            push_svg_number(&mut svg, self.logical_height, "SVG background height")?;
            svg.push_str("\" fill=\"");
            background.write_hex(&mut svg).map_err(|_| {
                FormatEncodeError::Encoding("failed to write SVG background color".to_owned())
            })?;
            if background.alpha() != 1.0 {
                svg.push_str("\" fill-opacity=\"");
                push_svg_number(&mut svg, background.alpha(), "SVG background alpha")?;
            }
            svg.push_str("\"/>");
        }
        for (element_index, element) in self.elements.iter().enumerate() {
            let frame_clip_count = element.frame_clips().len();
            // The name precedes the frame as its own element, outside the
            // frame's clip and link, where the reference export puts it.
            if let SvgElement::Frame {
                source,
                name: Some(name),
                ..
            } = element
            {
                push_frame_name(&mut svg, source, name)?;
            }
            push_frame_clip_prefix(&mut svg, element.frame_clips())?;
            if let Some(link) = element.link() {
                svg.push_str("<a href=\"");
                push_escaped_attribute(&mut svg, link);
                svg.push_str("\">");
            }
            match element {
                SvgElement::RoughOutline {
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
                    label_mask,
                    generated_label,
                    generated_label_color,
                    generated_label_offset_y,
                    link,
                    width,
                    height,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG plan outline lost its source identity".to_owned(),
                        ));
                    }
                    if let Some(mask) = label_mask {
                        svg.push_str("<defs><mask id=\"arrow-label-mask-");
                        write!(svg, "{element_index}").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG arrow label mask ID".to_owned(),
                            )
                        })?;
                        svg.push_str("\" maskUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"");
                        push_svg_number(&mut svg, self.logical_width, "SVG label mask width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, self.logical_height, "SVG label mask height")?;
                        svg.push_str(
                            "\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/><rect x=\"",
                        );
                        push_svg_number(&mut svg, mask.x, "SVG label mask x")?;
                        svg.push_str("\" y=\"");
                        push_svg_number(&mut svg, mask.y, "SVG label mask y")?;
                        svg.push_str("\" width=\"");
                        push_svg_number(&mut svg, mask.width, "SVG label mask rectangle width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, mask.height, "SVG label mask rectangle height")?;
                        svg.push_str(
                            "\" fill=\"black\"/></mask></defs><g mask=\"url(#arrow-label-mask-",
                        );
                        write!(svg, "{element_index}").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG arrow label mask reference".to_owned(),
                            )
                        })?;
                        svg.push_str(")\">");
                    }
                    svg.push_str("<g stroke-linecap=\"round\"");
                    svg.push_str(" transform=\"translate(");
                    push_svg_number(&mut svg, transform.translation_x, "SVG translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.translation_y, "SVG translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(&mut svg, transform.rotation_degrees, "SVG rotation")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_x, "SVG rotation center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_y, "SVG rotation center y")?;
                    svg.push_str(")\"");
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    push_svg_drawing(
                        &mut svg,
                        drawing,
                        *stroke_color,
                        *fill_color,
                        *stroke_width,
                        *stroke_style,
                        *source_stroke_width,
                        *opacity,
                    )?;
                    for decoration in &drawing.decorations {
                        let decoration_fill = super::resolve::decoration_fill_color(
                            decoration,
                            *stroke_color,
                            *background_color,
                        );
                        let (decoration_style, decoration_stroke_width) =
                            super::resolve::decoration_stroke_style(
                                decoration,
                                *source_stroke_width,
                            );
                        push_svg_drawing(
                            &mut svg,
                            &decoration.drawing,
                            *stroke_color,
                            Some(decoration_fill),
                            *stroke_width,
                            decoration_style,
                            decoration_stroke_width,
                            *opacity,
                        )?;
                    }
                    if let (Some(label), Some(color)) = (generated_label, generated_label_color) {
                        svg.push_str("<text data-source-text=\"");
                        for line in &label.lines {
                            push_escaped_attribute(&mut svg, &line.text);
                        }
                        svg.push_str("\" xml:space=\"preserve\" fill=\"");
                        color.write_hex(&mut svg).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG embeddable label color".to_owned(),
                            )
                        })?;
                        svg.push_str("\" font-family=\"ExcalidrawRender-9\" font-size=\"");
                        push_svg_number(&mut svg, label.font_size, "SVG embeddable font size")?;
                        svg.push_str("\">");
                        for line in &label.lines {
                            if !line.runs.is_empty() {
                                for run in &line.runs {
                                    svg.push_str("<tspan font-family=\"");
                                    push_escaped_attribute(
                                        &mut svg,
                                        super::svg_fonts::Bundle::effective_family(run.font_family),
                                    );
                                    svg.push_str(if run.rtl {
                                        "\" direction=\"rtl\""
                                    } else {
                                        "\" direction=\"ltr\""
                                    });
                                    push_svg_run_positions(
                                        &mut svg,
                                        line,
                                        run,
                                        *generated_label_offset_y,
                                        "SVG embeddable run",
                                    )?;
                                    svg.push('>');
                                    push_escaped_text(&mut svg, &run.display_text);
                                    svg.push_str("</tspan>");
                                }
                                continue;
                            }
                            svg.push_str("<tspan x=\"");
                            for (index, glyph) in line.glyphs.iter().enumerate() {
                                if index != 0 {
                                    svg.push(' ');
                                }
                                push_svg_number(&mut svg, glyph.x, "SVG embeddable glyph x")?;
                            }
                            svg.push_str("\" y=\"");
                            if line.glyphs.is_empty() {
                                push_svg_number(
                                    &mut svg,
                                    line.baseline + generated_label_offset_y,
                                    "SVG embeddable baseline",
                                )?;
                            } else {
                                for (index, glyph) in line.glyphs.iter().enumerate() {
                                    if index != 0 {
                                        svg.push(' ');
                                    }
                                    push_svg_number(
                                        &mut svg,
                                        glyph.y + generated_label_offset_y,
                                        "SVG embeddable glyph y",
                                    )?;
                                }
                            }
                            svg.push_str("\">");
                            push_escaped_text(&mut svg, &line.display_text);
                            svg.push_str("</tspan>");
                        }
                        svg.push_str("</text>");
                    }
                    svg.push_str("</g>");
                    if label_mask.is_some() {
                        svg.push_str("</g>");
                    }
                }
                SvgElement::Freedraw {
                    source,
                    path_data,
                    background_fill,
                    frame_clips: _,
                    transform,
                    color,
                    background_color,
                    source_stroke_width,
                    opacity,
                    link,
                    width,
                    height,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG plan freedraw lost its source identity".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(
                        &mut svg,
                        transform.translation_x,
                        "SVG freedraw translation x",
                    )?;
                    svg.push(' ');
                    push_svg_number(
                        &mut svg,
                        transform.translation_y,
                        "SVG freedraw translation y",
                    )?;
                    svg.push_str(") rotate(");
                    push_svg_number(
                        &mut svg,
                        transform.rotation_degrees,
                        "SVG freedraw rotation",
                    )?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_x, "SVG freedraw center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_y, "SVG freedraw center y")?;
                    svg.push_str(")\">");
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    if let (Some(fill), Some(background_color)) =
                        (background_fill, background_color)
                    {
                        svg.push_str("<path d=\"");
                        push_rough_path(&mut svg, &fill.path)?;
                        match fill.kind {
                            FillKind::Solid => {
                                svg.push_str("\" stroke=\"none\" fill=\"");
                                background_color.write_hex(&mut svg).map_err(|_| {
                                    FormatEncodeError::Encoding(
                                        "failed to write SVG freedraw background".to_owned(),
                                    )
                                })?;
                                svg.push_str("\" fill-rule=\"evenodd\"");
                                let fill_opacity = background_color.alpha() * opacity;
                                if fill_opacity != 1.0 {
                                    svg.push_str(" fill-opacity=\"");
                                    push_svg_number(
                                        &mut svg,
                                        fill_opacity,
                                        "SVG freedraw background opacity",
                                    )?;
                                    svg.push('"');
                                }
                            }
                            FillKind::Sketch => {
                                svg.push_str("\" stroke=\"");
                                background_color.write_hex(&mut svg).map_err(|_| {
                                    FormatEncodeError::Encoding(
                                        "failed to write SVG freedraw sketch fill".to_owned(),
                                    )
                                })?;
                                svg.push_str("\" stroke-width=\"");
                                push_svg_number(
                                    &mut svg,
                                    source_stroke_width / 2.0,
                                    "SVG freedraw fill weight",
                                )?;
                                svg.push_str("\" fill=\"none\"");
                                let fill_opacity = background_color.alpha() * opacity;
                                if fill_opacity != 1.0 {
                                    svg.push_str(" stroke-opacity=\"");
                                    push_svg_number(
                                        &mut svg,
                                        fill_opacity,
                                        "SVG freedraw sketch opacity",
                                    )?;
                                    svg.push('"');
                                }
                            }
                        }
                        svg.push_str("/> ");
                    }
                    svg.push_str("<path d=\"");
                    svg.push_str(path_data);
                    svg.push_str("\" fill=\"");
                    color.write_hex(&mut svg).map_err(|_| {
                        FormatEncodeError::Encoding("failed to write SVG freedraw color".to_owned())
                    })?;
                    let effective_opacity = color.alpha() * opacity;
                    if effective_opacity != 1.0 {
                        svg.push_str("\" fill-opacity=\"");
                        push_svg_number(
                            &mut svg,
                            effective_opacity,
                            "SVG freedraw effective opacity",
                        )?;
                    }
                    svg.push_str("\"/></g>");
                }
                SvgElement::Frame {
                    source,
                    name: _,
                    outline_stroke_color,
                    frame_clips: _,
                    transform,
                    opacity,
                    link,
                    width,
                    height,
                } => {
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, transform.translation_x, "SVG frame translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.translation_y, "SVG frame translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(&mut svg, transform.rotation_degrees, "SVG frame rotation")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_x, "SVG frame center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_y, "SVG frame center y")?;
                    svg.push_str(")\"");
                    if *opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG frame opacity")?;
                        svg.push('"');
                    }
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    if let Some(stroke_color) = outline_stroke_color {
                        // The reference outline: the frame rectangle with
                        // `FRAME_STYLE.radius`, `strokeColor` and `strokeWidth`.
                        svg.push_str("<rect data-frame-outline=\"");
                        push_escaped_attribute(&mut svg, source.id());
                        svg.push_str("\" width=\"");
                        push_svg_number(&mut svg, *width, "SVG frame outline width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, *height, "SVG frame outline height")?;
                        svg.push('"');
                        push_svg_corner_radius(&mut svg, super::geometry::FRAME_RADIUS)?;
                        svg.push_str(" fill=\"none\" stroke=\"");
                        stroke_color.write_hex(&mut svg).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG frame outline stroke".to_owned(),
                            )
                        })?;
                        svg.push_str("\" stroke-width=\"");
                        push_svg_number(
                            &mut svg,
                            super::geometry::FRAME_OUTLINE_STROKE_WIDTH,
                            "SVG frame outline stroke width",
                        )?;
                        svg.push_str("\"/>");
                    }
                    svg.push_str("</g>");
                }
                SvgElement::Text {
                    source,
                    text,
                    frame_clips: _,
                    transform,
                    color,
                    opacity,
                    link,
                    width,
                    height,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG text lost its source identity".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, transform.translation_x, "SVG text translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.translation_y, "SVG text translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(
                        &mut svg,
                        180.0 * transform.angle_radians / std::f64::consts::PI,
                        "SVG text rotation",
                    )?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_x, "SVG text center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_y, "SVG text center y")?;
                    svg.push_str(")\"");
                    let effective_opacity = opacity * color.alpha();
                    if effective_opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, effective_opacity, "SVG text opacity")?;
                        svg.push('"');
                    }
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    svg.push_str("<text data-source-text=\"");
                    for line in &text.lines {
                        push_escaped_attribute(&mut svg, &line.text);
                    }
                    svg.push_str("\" xml:space=\"preserve\" fill=\"");
                    color.write_hex(&mut svg).map_err(|_| {
                        FormatEncodeError::Encoding("failed to write SVG text color".to_owned())
                    })?;
                    svg.push_str("\" font-family=\"");
                    push_escaped_attribute(
                        &mut svg,
                        super::svg_fonts::Bundle::effective_family(text.font_family),
                    );
                    svg.push_str("\" font-size=\"");
                    push_svg_number(&mut svg, text.font_size, "SVG text font size")?;
                    let (line_x, text_anchor) = match text.text_align {
                        super::text::ResolvedTextAlign::Left => (0.0, "start"),
                        super::text::ResolvedTextAlign::Center => (*width / 2.0, "middle"),
                        super::text::ResolvedTextAlign::Right => (*width, "end"),
                    };
                    svg.push_str("\" text-anchor=\"");
                    svg.push_str(text_anchor);
                    svg.push_str("\">");
                    for line in &text.lines {
                        if line.runs.is_empty() {
                            svg.push_str("<tspan x=\"");
                            push_svg_number(&mut svg, line_x, "SVG text line anchor x")?;
                            svg.push_str("\" y=\"");
                            push_svg_number(&mut svg, line.baseline, "SVG text line baseline")?;
                            svg.push_str("\">");
                            push_escaped_text(&mut svg, &line.display_text);
                            svg.push_str("</tspan>");
                        } else {
                            for run in &line.runs {
                                svg.push_str("<tspan font-family=\"");
                                push_escaped_attribute(
                                    &mut svg,
                                    super::svg_fonts::Bundle::effective_family(run.font_family),
                                );
                                svg.push_str(if run.rtl {
                                    "\" direction=\"rtl\" text-anchor=\"start\""
                                } else {
                                    "\" direction=\"ltr\" text-anchor=\"start\""
                                });
                                push_svg_run_positions(&mut svg, line, run, 0.0, "SVG text run")?;
                                svg.push('>');
                                push_escaped_text(&mut svg, &run.display_text);
                                svg.push_str("</tspan>");
                            }
                        }
                        // Nothing separates one line's tspans from the next.
                        // Under xml:space="preserve" a separator is a rendered
                        // character that joins the preceding text chunk, and a
                        // trailing space shifts every middle- or end-anchored
                        // line off the position the official export gives it.
                        // The source line break is carried by data-source-text.
                    }
                    svg.push_str("</text></g>");
                }
                SvgElement::Image {
                    source,
                    asset,
                    dark_mode_filter,
                    corner_radius,
                    transform,
                    opacity,
                    link,
                    width,
                    height,
                    projection,
                    scale,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG image lost its source identity".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(&mut svg, transform.translation_x, "SVG image translation x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.translation_y, "SVG image translation y")?;
                    svg.push_str(") rotate(");
                    push_svg_number(
                        &mut svg,
                        180.0 * transform.angle_radians / std::f64::consts::PI,
                        "SVG image rotation",
                    )?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_x, "SVG image center x")?;
                    svg.push(' ');
                    push_svg_number(&mut svg, transform.center_y, "SVG image center y")?;
                    svg.push_str(")\"");
                    if corner_radius.is_some() {
                        svg.push_str(" clip-path=\"url(#");
                        push_image_clip_id(&mut svg, source)?;
                        svg.push_str(")\"");
                    }
                    if *opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG image opacity")?;
                        svg.push('"');
                    }
                    svg.push('>');
                    push_svg_link_hit_rect(&mut svg, link.as_deref(), *width, *height)?;
                    // The reference's image geometry (spec 8.6), inlined: its
                    // `<use>` is the rounded viewport, flipped about the
                    // element's unrounded center; an uncropped symbol image
                    // fills the viewport, a cropped one is drawn at its
                    // unrounded uncropped size under the crop offset, behind a
                    // viewport-sized `<mask>` on the group outside the flip.
                    let (_, _, content_width, content_height) = projection.content_rect();
                    if projection.crop.is_some() {
                        svg.push_str("<svg width=\"");
                        push_svg_number(
                            &mut svg,
                            projection.viewport_width,
                            "SVG image crop mask width",
                        )?;
                        svg.push_str("\" height=\"");
                        push_svg_number(
                            &mut svg,
                            projection.viewport_height,
                            "SVG image crop mask height",
                        )?;
                        svg.push_str("\" overflow=\"hidden\">");
                    }
                    if *scale != [1, 1] {
                        svg.push_str("<g transform=\"translate(");
                        push_svg_number(
                            &mut svg,
                            if scale[0] < 0 { *width } else { 0.0 },
                            "SVG image flip translation x",
                        )?;
                        svg.push(' ');
                        push_svg_number(
                            &mut svg,
                            if scale[1] < 0 { *height } else { 0.0 },
                            "SVG image flip translation y",
                        )?;
                        svg.push_str(") scale(");
                        write!(svg, "{} {}", scale[0], scale[1]).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write SVG image flip scale".to_owned(),
                            )
                        })?;
                        svg.push_str(")\">");
                    }
                    if projection.crop.is_some() {
                        // The `<use>` viewport, whose origin is also where the
                        // symbol image inside it is drawn.
                        let (viewport_x, viewport_y, viewport_width, viewport_height) =
                            projection.viewport_rect();
                        svg.push_str("<svg x=\"");
                        push_svg_number(&mut svg, viewport_x, "SVG image viewport x")?;
                        svg.push_str("\" y=\"");
                        push_svg_number(&mut svg, viewport_y, "SVG image viewport y")?;
                        svg.push_str("\" width=\"");
                        push_svg_number(&mut svg, viewport_width, "SVG image viewport width")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, viewport_height, "SVG image viewport height")?;
                        svg.push_str("\" overflow=\"hidden\">");
                    }
                    svg.push_str("<image href=\"data:");
                    svg.push_str(asset.media_type);
                    svg.push_str(";base64,");
                    push_base64(&mut svg, &asset.source_bytes)?;
                    svg.push_str("\" preserveAspectRatio=\"none\" width=\"");
                    push_svg_number(&mut svg, content_width, "SVG image width")?;
                    svg.push_str("\" height=\"");
                    push_svg_number(&mut svg, content_height, "SVG image height")?;
                    if let Some(filter) = dark_mode_filter {
                        svg.push_str("\" filter=\"url(#");
                        svg.push_str(filter.id());
                        svg.push(')');
                    }
                    svg.push_str("\"/>");
                    if projection.crop.is_some() {
                        svg.push_str("</svg>");
                    }
                    if *scale != [1, 1] {
                        svg.push_str("</g>");
                    }
                    if projection.crop.is_some() {
                        svg.push_str("</svg>");
                    }
                    svg.push_str("</g>");
                }
                SvgElement::Fallback {
                    source,
                    clip_index,
                    label,
                    link: _,
                    frame_clips: _,
                    width,
                    height,
                    transform,
                    opacity,
                } => {
                    if source.pointer.is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "SVG fallback lost its source location".to_owned(),
                        ));
                    }
                    svg.push_str("<g transform=\"translate(");
                    push_svg_number(
                        &mut svg,
                        transform.translation_x,
                        "SVG fallback translation x",
                    )?;
                    svg.push(' ');
                    push_svg_number(
                        &mut svg,
                        transform.translation_y,
                        "SVG fallback translation y",
                    )?;
                    svg.push_str(") rotate(");
                    push_svg_number(
                        &mut svg,
                        transform.rotation_degrees,
                        "SVG fallback rotation",
                    )?;
                    svg.push(' ');
                    push_svg_number(
                        &mut svg,
                        transform.center_x,
                        "SVG fallback rotation center x",
                    )?;
                    svg.push(' ');
                    push_svg_number(
                        &mut svg,
                        transform.center_y,
                        "SVG fallback rotation center y",
                    )?;
                    svg.push_str(")\"");
                    if *opacity != 1.0 {
                        svg.push_str(" opacity=\"");
                        push_svg_number(&mut svg, *opacity, "SVG fallback opacity")?;
                        svg.push('"');
                    }
                    svg.push_str("><clipPath id=\"fallback-");
                    write!(svg, "{clip_index}").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write SVG fallback clip identity".to_owned(),
                        )
                    })?;
                    svg.push_str("\"><rect x=\"0\" y=\"0\" width=\"");
                    push_svg_number(&mut svg, *width, "SVG fallback clip width")?;
                    svg.push_str("\" height=\"");
                    push_svg_number(&mut svg, *height, "SVG fallback clip height")?;
                    svg.push_str("\"/></clipPath><g clip-path=\"url(#fallback-");
                    write!(svg, "{clip_index}").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write SVG fallback clip reference".to_owned(),
                        )
                    })?;
                    svg.push_str(")\"><rect x=\"0\" y=\"0\" width=\"");
                    push_svg_number(&mut svg, *width, "SVG fallback fill width")?;
                    svg.push_str("\" height=\"");
                    push_svg_number(&mut svg, *height, "SVG fallback fill height")?;
                    svg.push_str("\" fill=\"");
                    self.fallback_colors.fill.write_hex(&mut svg).map_err(|_| {
                        FormatEncodeError::Encoding("failed to write SVG fallback fill".to_owned())
                    })?;
                    svg.push_str("\"/>");
                    if *width >= 2.0 && *height >= 2.0 {
                        svg.push_str("<rect x=\"0\" y=\"0\" width=\"");
                        push_svg_number(&mut svg, *width, "SVG fallback stroke width extent")?;
                        svg.push_str("\" height=\"");
                        push_svg_number(&mut svg, *height, "SVG fallback stroke height extent")?;
                        svg.push_str("\" fill=\"none\" stroke=\"");
                        self.fallback_colors
                            .stroke
                            .write_hex(&mut svg)
                            .map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write SVG fallback stroke".to_owned(),
                                )
                            })?;
                        svg.push_str(concat!(
                            "\" stroke-width=\"2\" stroke-dasharray=\"6 4\" ",
                            "stroke-dashoffset=\"0\"/><path d=\"M0 0 L"
                        ));
                        push_svg_number(&mut svg, *width, "SVG fallback cross width")?;
                        svg.push(' ');
                        push_svg_number(&mut svg, *height, "SVG fallback cross height")?;
                        svg.push_str(" M");
                        push_svg_number(&mut svg, *width, "SVG fallback reverse cross width")?;
                        svg.push_str(" 0 L0 ");
                        push_svg_number(&mut svg, *height, "SVG fallback reverse cross height")?;
                        svg.push_str("\" fill=\"none\" stroke=\"");
                        self.fallback_colors
                            .stroke
                            .write_hex(&mut svg)
                            .map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write SVG fallback cross".to_owned(),
                                )
                            })?;
                        svg.push_str("\" stroke-width=\"2\"/>");
                    }
                    if let Some(label) = label {
                        svg.push_str("<text x=\"");
                        push_svg_number(&mut svg, *width / 2.0, "SVG fallback label x")?;
                        svg.push_str("\" y=\"");
                        push_svg_number(&mut svg, *height / 2.0, "SVG fallback label y")?;
                        svg.push_str("\" fill=\"");
                        self.fallback_colors
                            .label
                            .write_hex(&mut svg)
                            .map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write SVG fallback label color".to_owned(),
                                )
                            })?;
                        svg.push_str(concat!(
                            "\" font-family=\"ExcalidrawRender-9\" font-size=\"14\" ",
                            "text-anchor=\"middle\" dominant-baseline=\"central\">"
                        ));
                        push_escaped_text(&mut svg, label);
                        svg.push_str("</text>");
                    }
                    svg.push_str("</g></g>");
                }
            }
            if element.link().is_some() {
                svg.push_str("</a>");
            }
            for _ in 0..frame_clip_count {
                svg.push_str("</g>");
            }
        }
        svg.push_str("</svg>");
        Ok(())
    }

    pub(super) fn set_metadata(&mut self, metadata: super::metadata::ArtifactMetadata) {
        self.metadata = Some(metadata);
        self.mandatory_notices = None;
    }

    pub(super) fn take_mandatory_notices(&mut self) -> Option<super::notices::MandatoryNotices> {
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

#[allow(clippy::too_many_arguments)]
fn push_svg_drawing<W: SvgSink + ?Sized>(
    output: &mut W,
    drawing: &Drawing,
    stroke_color: CanonicalColor,
    fill_color: Option<CanonicalColor>,
    stroke_width: f64,
    stroke_style: StrokeStyle,
    source_stroke_width: f64,
    opacity: f64,
) -> Result<(), FormatEncodeError> {
    if let (Some(fill), Some(fill_color)) = (&drawing.fill, fill_color) {
        output.push_str("<path d=\"");
        push_rough_path(output, &fill.path)?;
        match fill.kind {
            FillKind::Solid => {
                output.push_str("\" stroke=\"none\" fill=\"");
                fill_color.write_hex(output).map_err(|_| {
                    FormatEncodeError::Encoding("failed to write SVG fill color".to_owned())
                })?;
                output.push('"');
                if drawing.shape.svg_uses_even_odd_fill() {
                    output.push_str(" fill-rule=\"evenodd\"");
                }
                if fill_color.alpha() * opacity != 1.0 {
                    output.push_str(" fill-opacity=\"");
                    push_svg_number(
                        output,
                        fill_color.alpha() * opacity,
                        "SVG effective fill alpha",
                    )?;
                    output.push('"');
                }
            }
            FillKind::Sketch => {
                output.push_str("\" stroke=\"");
                fill_color.write_hex(output).map_err(|_| {
                    FormatEncodeError::Encoding("failed to write SVG sketch color".to_owned())
                })?;
                output.push_str("\" stroke-width=\"");
                push_svg_number(output, source_stroke_width / 2.0, "SVG fill weight")?;
                output.push_str("\" fill=\"none\"");
                if fill_color.alpha() * opacity != 1.0 {
                    output.push_str(" stroke-opacity=\"");
                    push_svg_number(
                        output,
                        fill_color.alpha() * opacity,
                        "SVG effective fill alpha",
                    )?;
                    output.push('"');
                }
            }
        }
        output.push_str("/> ");
    }
    output.push_str("<path d=\"");
    push_rough_path(output, &drawing.outline)?;
    output.push_str("\" stroke=\"");
    stroke_color
        .write_hex(output)
        .map_err(|_| FormatEncodeError::Encoding("failed to write SVG stroke color".to_owned()))?;
    output.push_str("\" stroke-width=\"");
    push_svg_number(output, stroke_width, "SVG stroke width")?;
    output.push_str("\" fill=\"none\"");
    match stroke_style {
        StrokeStyle::Solid => {}
        StrokeStyle::Dashed => {
            output.push_str(" stroke-dasharray=\"8 ");
            push_svg_number(output, 8.0 + source_stroke_width, "SVG dash gap")?;
            output.push('"');
        }
        StrokeStyle::Dotted => {
            output.push_str(" stroke-dasharray=\"1.5 ");
            push_svg_number(output, 6.0 + source_stroke_width, "SVG dot gap")?;
            output.push('"');
        }
    }
    if stroke_color.alpha() * opacity != 1.0 {
        output.push_str(" stroke-opacity=\"");
        push_svg_number(
            output,
            stroke_color.alpha() * opacity,
            "SVG effective stroke alpha",
        )?;
        output.push('"');
    }
    output.push_str("/>");
    Ok(())
}

/// Writes a Rough.js path as SVG path data: the operations space-separated,
/// each coordinate in the two-decimal form Rough.js emits.
pub(super) fn push_rough_path<W: SvgSink + ?Sized>(
    output: &mut W,
    path: &Path,
) -> Result<(), FormatEncodeError> {
    for (index, operation) in path.operations.iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        match operation {
            PathOperation::Move(point) => {
                output.push('M');
                super::rough::push_number(output, point.x)?;
                output.push(' ');
                super::rough::push_number(output, point.y)?;
            }
            PathOperation::Line(point) => {
                output.push('L');
                super::rough::push_number(output, point.x)?;
                output.push(' ');
                super::rough::push_number(output, point.y)?;
            }
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                output.push('C');
                super::rough::push_number(output, control_1.x)?;
                output.push(' ');
                super::rough::push_number(output, control_1.y)?;
                output.push_str(", ");
                super::rough::push_number(output, control_2.x)?;
                output.push(' ');
                super::rough::push_number(output, control_2.y)?;
                output.push_str(", ");
                super::rough::push_number(output, end.x)?;
                output.push(' ');
                super::rough::push_number(output, end.y)?;
            }
        }
    }
    Ok(())
}

/// The SVG path data for a freedraw outline, in the form Excalidraw's
/// `getSvgPathFromStroke` emits: `M x,y Q` then control/end pairs, `L` for a
/// straight segment and `Z` to close, every coordinate cut to two decimals.
pub(super) fn freedraw_path_data(
    path: &super::freedraw::Path,
) -> Result<String, FormatEncodeError> {
    let mut output = String::new();
    output
        .try_reserve(path.operations.len().saturating_mul(64))
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_plan",
            subject: "svg_freedraw_path_data",
        })?;
    for operation in &path.operations {
        match *operation {
            super::freedraw::PathOperation::Move(point) => {
                output.push_str("M ");
                push_freedraw_point(&mut output, point)?;
                output.push_str(" Q");
            }
            super::freedraw::PathOperation::Quadratic { control, end } => {
                output.push(' ');
                push_freedraw_point(&mut output, control)?;
                output.push(' ');
                push_freedraw_point(&mut output, end)?;
            }
            super::freedraw::PathOperation::Line(point) => {
                output.push_str(" L ");
                push_freedraw_point(&mut output, point)?;
            }
            super::freedraw::PathOperation::Close => output.push_str(" Z"),
        }
    }
    Ok(output)
}

fn push_freedraw_point(
    output: &mut String,
    point: super::rough::Point,
) -> Result<(), FormatEncodeError> {
    push_freedraw_number(output, point.x)?;
    output.push(',');
    push_freedraw_number(output, point.y)
}

fn push_freedraw_number(output: &mut String, value: f64) -> Result<(), FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(
            "SVG freedraw path contains a non-finite coordinate".to_owned(),
        ));
    }
    let token = super::freedraw::ecmascript_number(value);
    let Some(decimal) = token.find('.') else {
        output.push_str(&token);
        return Ok(());
    };
    let digits = token[decimal + 1..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count()
        .min(2);
    let retained_end = decimal + 1 + digits;
    let consumed_end = token[retained_end..]
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || matches!(byte, b'e' | b'-'))
        .count()
        + retained_end;
    output.push_str(&token[..retained_end]);
    output.push_str(&token[consumed_end..]);
    Ok(())
}

fn push_svg_number<W: SvgSink + ?Sized>(
    output: &mut W,
    value: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(format!(
            "{subject} must be finite"
        )));
    }
    let value = if value == 0.0 { 0.0 } else { value };
    write!(output, "{value}")
        .map_err(|_| FormatEncodeError::Encoding(format!("failed to write {subject}")))
}

fn push_escaped_attribute<W: SvgSink + ?Sized>(output: &mut W, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '"' => output.push_str("&quot;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\n' => output.push_str("&#10;"),
            '\r' => output.push_str("&#13;"),
            '\t' => output.push_str("&#9;"),
            character => output.push(character),
        }
    }
}

fn push_escaped_text<W: SvgSink + ?Sized>(output: &mut W, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            character => output.push(character),
        }
    }
}

fn push_base64<W: SvgSink + ?Sized>(output: &mut W, bytes: &[u8]) -> Result<(), FormatEncodeError> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(ALPHABET[usize::from(first >> 2)]));
        output.push(char::from(
            ALPHABET[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        if chunk.len() > 1 {
            output.push(char::from(
                ALPHABET[usize::from(((second & 0x0f) << 2) | (third >> 6))],
            ));
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(char::from(ALPHABET[usize::from(third & 0x3f)]));
        } else {
            output.push('=');
        }
    }
    Ok(())
}

fn svg_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("SVG plan exceeds serializer capacity".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{push_escaped_attribute, push_escaped_text};

    /// `data-source-text` carries a text element's own bytes: the markup
    /// characters and every line-ending or tab byte are escaped as character
    /// references so a `\r` survives attribute-value normalization (#110).
    #[test]
    fn attribute_escaping_keeps_line_endings_as_character_references() {
        let escaped = |value: &str| {
            let mut output = String::new();
            push_escaped_attribute(&mut output, value);
            output
        };
        assert_eq!(escaped("a\nb"), "a&#10;b");
        assert_eq!(escaped("a\r\nb"), "a&#13;&#10;b");
        assert_eq!(escaped("a\rb"), "a&#13;b");
        assert_eq!(escaped("\t<\"&\">"), "&#9;&lt;&quot;&amp;&quot;&gt;");
        assert_eq!(escaped("plain ünïcödé"), "plain ünïcödé");
    }

    /// Text content escapes only the markup characters; a quote is literal
    /// there, and whitespace is preserved as written under
    /// `xml:space="preserve"`.
    #[test]
    fn text_escaping_leaves_quotes_and_whitespace_literal() {
        let mut output = String::new();
        push_escaped_text(&mut output, "a \"b\" <c> & d\t");
        assert_eq!(output, "a \"b\" &lt;c&gt; &amp; d\t");
    }
}
