use std::fmt::Write as _;
use std::io::Write as _;

use crate::{
    color::CanonicalColor, FiniteValue, LogicalBounds, NestedPath, NestedPathSegment,
    NestedSegmentKind, RenderRequest,
};

use super::{
    rough::{self, Drawing, FillKind, Path, PathOperation},
    FormatEncodeError, NormalizedElementSource, ResolvedElement, ResolvedScene, StrokeStyle,
};

pub(super) const POINTS_PER_LOGICAL_PIXEL: f64 = 0.75;
const ICC_PROFILE_OBJECT: usize = 5;
const OUTPUT_INTENT_OBJECT: usize = 6;
const FIRST_EXT_GSTATE_OBJECT: usize = 7;

// Exact sRGB device-to-linear values, frozen as profile data rather than
// recalculated with target-dependent transcendental functions.
const SRGB_TRC: [u16; 256] = [
    0, 20, 40, 60, 80, 99, 119, 139, 159, 179, 199, 219, 241, 264, 288, 313, 340, 367, 396, 427,
    458, 491, 526, 562, 599, 637, 677, 718, 761, 805, 851, 898, 947, 997, 1048, 1101, 1156, 1212,
    1270, 1330, 1391, 1453, 1517, 1583, 1651, 1720, 1790, 1863, 1937, 2013, 2090, 2170, 2250, 2333,
    2418, 2504, 2592, 2681, 2773, 2866, 2961, 3058, 3157, 3258, 3360, 3464, 3570, 3678, 3788, 3900,
    4014, 4129, 4247, 4366, 4488, 4611, 4736, 4864, 4993, 5124, 5257, 5392, 5530, 5669, 5810, 5953,
    6099, 6246, 6395, 6547, 6700, 6856, 7014, 7174, 7335, 7500, 7666, 7834, 8004, 8177, 8352, 8528,
    8708, 8889, 9072, 9258, 9445, 9635, 9828, 10022, 10219, 10417, 10619, 10822, 11028, 11235,
    11446, 11658, 11873, 12090, 12309, 12530, 12754, 12980, 13209, 13440, 13673, 13909, 14146,
    14387, 14629, 14874, 15122, 15371, 15623, 15878, 16135, 16394, 16656, 16920, 17187, 17456,
    17727, 18001, 18277, 18556, 18837, 19121, 19407, 19696, 19987, 20281, 20577, 20876, 21177,
    21481, 21787, 22096, 22407, 22721, 23038, 23357, 23678, 24002, 24329, 24658, 24990, 25325,
    25662, 26001, 26344, 26688, 27036, 27386, 27739, 28094, 28452, 28813, 29176, 29542, 29911,
    30282, 30656, 31033, 31412, 31794, 32179, 32567, 32957, 33350, 33745, 34143, 34544, 34948,
    35355, 35764, 36176, 36591, 37008, 37429, 37852, 38278, 38706, 39138, 39572, 40009, 40449,
    40891, 41337, 41785, 42236, 42690, 43147, 43606, 44069, 44534, 45002, 45473, 45947, 46423,
    46903, 47385, 47871, 48359, 48850, 49344, 49841, 50341, 50844, 51349, 51858, 52369, 52884,
    53401, 53921, 54445, 54971, 55500, 56032, 56567, 57105, 57646, 58190, 58737, 59287, 59840,
    60396, 60955, 61517, 62082, 62650, 63221, 63795, 64372, 64952, 65535,
];

// Project-owned content is lowered directly; the bounded svg2pdf adapter below
// is confined to sanitized nested-SVG Form chunks.
pub(super) struct PdfPlan {
    logical_width: f64,
    logical_height: f64,
    scale: f64,
    page_width: f64,
    page_height: f64,
    background: Option<CanonicalColor>,
    fallback_colors: super::FallbackColors,
    mandatory_notices: Option<String>,
    metadata: Option<super::metadata::ArtifactMetadata>,
    fallback_font_sfnt: Option<Vec<u8>>,
    official_fonts: Vec<super::ResolvedFont>,
    elements: Vec<PdfElement>,
    raster_closures: Vec<PdfRasterClosure>,
}

pub(super) struct PdfRasterClosure {
    pub(super) element_id: String,
    pub(super) json_pointer: String,
    pub(super) source_order: u64,
    pub(super) source_position: u64,
    pub(super) logical_bounds: LogicalBounds,
    pub(super) pixel_width: u64,
    pub(super) pixel_height: u64,
    pub(super) causes: Vec<NestedPath>,
    pub(super) unisolatable: bool,
}

struct PdfSvgResource {
    asset: std::sync::Arc<super::ResolvedImageAsset>,
    raster_scale: f64,
}

struct FilterOccurrence {
    filter_id: String,
    usage_id: String,
    marker: String,
    bounds: (f64, f64, f64, f64),
    parent: Option<usize>,
}

enum PdfElement {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        frame_clips: Vec<super::ResolvedClip>,
        transform: super::ResolvedTransform,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
        background_color: CanonicalColor,
        label_mask: Option<super::ResolvedLabelMask>,
        link: Option<String>,
        width: f64,
        height: f64,
        generated_label: Option<super::text::ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
    },
    Freedraw {
        source: NormalizedElementSource,
        path: super::freedraw::ProjectedPath,
        background_fill: Option<super::rough::Fill>,
        frame_clips: Vec<super::ResolvedClip>,
        transform: super::ResolvedTransform,
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
        name: Option<super::text::ResolvedLabel>,
        magic: bool,
        outline_path: Option<Path>,
        frame_clips: Vec<super::ResolvedClip>,
        transform: super::ResolvedTransform,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Text {
        source: NormalizedElementSource,
        text: super::text::ResolvedText,
        frame_clips: Vec<super::ResolvedClip>,
        transform: super::ResolvedTransform,
        color: CanonicalColor,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
    },
    Image {
        source: NormalizedElementSource,
        asset: std::sync::Arc<super::ResolvedImageAsset>,
        frame_clips: Vec<super::ResolvedClip>,
        transform: super::ResolvedTransform,
        opacity: f64,
        link: Option<String>,
        width: f64,
        height: f64,
        source_rect: super::ResolvedImageSourceRect,
        scale: [i8; 2],
    },
    Fallback {
        source: super::FallbackSource,
        label: Option<super::text::ResolvedLabel>,
        link: Option<String>,
        frame_clips: Vec<super::ResolvedClip>,
        width: f64,
        height: f64,
        transform: super::ResolvedTransform,
        opacity: f64,
    },
}

struct PdfGlyphMapping {
    code: u16,
    glyph_id: u16,
    advance_1000: u16,
    unicode: Vec<u16>,
}

struct PdfSvgChunk {
    chunk: pdf_writer::Chunk,
    root: pdf_writer::Ref,
    bytes: Vec<u8>,
    object_offsets: Vec<(usize, usize)>,
}

impl PdfPlan {
    pub(super) fn image_resource_pixel_counts(&self) -> Result<Vec<u64>, FormatEncodeError> {
        let mut counts = self
            .image_resources()?
            .iter()
            .map(|asset| {
                u64::from(asset.width)
                    .checked_mul(u64::from(asset.height))
                    .ok_or_else(pdf_size_error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        counts.extend(
            self.raster_closures
                .iter()
                .map(|closure| closure.pixel_width.saturating_mul(closure.pixel_height)),
        );
        Ok(counts)
    }

    pub(super) fn new(
        scene: ResolvedScene,
        request: &RenderRequest,
    ) -> Result<Self, FormatEncodeError> {
        let scale = scene.raster_or_pdf_scale(request.sizing());
        let (page_width, page_height) = page_dimensions(&scene, request)?;
        let background = scene.include_background.then_some(scene.background_color);
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(scene.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "pdf_elements",
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
                    let frame_clips = if outline.generated_label.is_some() {
                        Vec::new()
                    } else {
                        outline.frame_clips
                    };
                    elements.push(PdfElement::RoughOutline {
                        source: outline.source,
                        drawing: outline.drawing,
                        frame_clips,
                        transform: outline.transform,
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
                        width: outline.width,
                        height: outline.height,
                        generated_label: outline.generated_label,
                        generated_label_color: outline.generated_label_color,
                        generated_label_offset_y: outline.generated_label_offset_y,
                    });
                }
                ResolvedElement::Freedraw(freedraw) => {
                    elements.push(PdfElement::Freedraw {
                        source: freedraw.source,
                        path: super::freedraw::project(freedraw.path)?,
                        background_fill: freedraw.background_fill,
                        frame_clips: freedraw.frame_clips,
                        transform: freedraw.transform,
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
                    elements.push(PdfElement::Frame {
                        source: frame.source,
                        name: frame.name,
                        magic: frame.magic,
                        outline_path: frame.outline_path,
                        frame_clips: frame.frame_clips,
                        transform: frame.transform,
                        opacity: frame.opacity,
                        link: frame.link,
                        width: frame.width,
                        height: frame.height,
                    });
                }
                ResolvedElement::Text(text) => {
                    elements.push(PdfElement::Text {
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
                    elements.push(PdfElement::Image {
                        source: image.source,
                        asset: image.asset,
                        frame_clips: image.frame_clips,
                        transform: image.transform,
                        opacity: image.opacity,
                        link: image.link,
                        width: image.width,
                        height: image.height,
                        source_rect: image.source_rect,
                        scale: image.scale,
                    });
                }
                ResolvedElement::ImageFallback(_) => {}
                ResolvedElement::Fallback(fallback) => {
                    elements.push(PdfElement::Fallback {
                        source: fallback.source,
                        label: fallback.label,
                        link: fallback.link,
                        frame_clips: fallback.frame_clips,
                        width: fallback.width,
                        height: fallback.height,
                        transform: fallback.transform,
                        opacity: fallback.opacity,
                    });
                }
            }
        }
        let raster_closures =
            collect_raster_closures(&elements, scale, scene.logical_width, scene.logical_height)?;
        Ok(Self {
            logical_width: scene.logical_width,
            logical_height: scene.logical_height,
            scale,
            page_width,
            page_height,
            background,
            fallback_colors: scene.fallback_colors,
            mandatory_notices: scene.mandatory_notices,
            metadata: None,
            fallback_font_sfnt: scene.fallback_font_sfnt,
            official_fonts: scene.official_fonts,
            elements,
            raster_closures,
        })
    }

    pub(super) fn raster_closures(&self) -> &[PdfRasterClosure] {
        &self.raster_closures
    }

    pub(super) fn take_mandatory_notices(&mut self) -> Option<String> {
        self.mandatory_notices.take()
    }

    pub(super) fn set_metadata(&mut self, metadata: super::metadata::ArtifactMetadata) {
        self.metadata = Some(metadata);
        self.mandatory_notices = None;
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let opacity_resources = self.opacity_resources()?;
        let image_resources = self.image_resources()?;
        let svg_resources = self.svg_resources()?;
        let raw_svg_chunks = svg_chunks(&svg_resources)?;
        let fallback_mappings = self.fallback_glyph_mappings()?;
        let official_mappings = self.official_glyph_mappings()?;
        let content = self.encode_content(
            &opacity_resources,
            &image_resources,
            &svg_resources,
            &fallback_mappings,
            &official_mappings,
        )?;
        let fallback_forms = self.encode_fallback_forms(&fallback_mappings)?;
        let xmp = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::manifest)
            .map(xmp_metadata)
            .transpose()?;
        let embedded_scene = self
            .metadata
            .as_ref()
            .and_then(super::metadata::ArtifactMetadata::canonical_scene);
        let attachment_object_count = usize::from(embedded_scene.is_some()) * 2;
        let mut link_count = 0_usize;
        for element in &self.elements {
            if let Some((_, width, height, transform, frame_clips)) = element.link_geometry() {
                if link_annotation_visible(
                    width,
                    height,
                    transform,
                    frame_clips,
                    self.scale * POINTS_PER_LOGICAL_PIXEL,
                    self.page_width,
                    self.page_height,
                )? {
                    link_count = link_count.checked_add(1).ok_or_else(pdf_size_error)?;
                }
            }
        }
        let srgb_icc_profile = srgb_icc_profile()?;
        let object_count = 6_usize
            .checked_add(opacity_resources.len())
            .and_then(|count| count.checked_add(usize::from(xmp.is_some())))
            .and_then(|count| count.checked_add(attachment_object_count))
            .and_then(|count| count.checked_add(link_count))
            .and_then(|count| count.checked_add(fallback_forms.len()))
            .ok_or_else(pdf_size_error)?;
        let metadata_object = if xmp.is_some() {
            Some(
                FIRST_EXT_GSTATE_OBJECT
                    .checked_add(opacity_resources.len())
                    .ok_or_else(pdf_size_error)?,
            )
        } else {
            None
        };
        let first_attachment_object = FIRST_EXT_GSTATE_OBJECT
            .checked_add(opacity_resources.len())
            .and_then(|object| object.checked_add(usize::from(xmp.is_some())))
            .ok_or_else(pdf_size_error)?;
        let attachment_filespec_object = embedded_scene
            .map(|_| {
                first_attachment_object
                    .checked_add(1)
                    .ok_or_else(pdf_size_error)
            })
            .transpose()?;
        let first_link_object = first_attachment_object
            .checked_add(attachment_object_count)
            .ok_or_else(pdf_size_error)?;
        let first_fallback_form_object = first_link_object
            .checked_add(link_count)
            .ok_or_else(pdf_size_error)?;
        let after_fallback_forms = first_fallback_form_object
            .checked_add(fallback_forms.len())
            .ok_or_else(pdf_size_error)?;
        let fallback_font_object = self
            .fallback_font_sfnt
            .as_ref()
            .map(|_| after_fallback_forms);
        let fallback_font_object_count = usize::from(fallback_font_object.is_some())
            .checked_mul(6)
            .ok_or_else(pdf_size_error)?;
        let official_font_object = if self.official_fonts.is_empty() {
            None
        } else {
            Some(
                after_fallback_forms
                    .checked_add(fallback_font_object_count)
                    .ok_or_else(pdf_size_error)?,
            )
        };
        let official_font_object_count = self
            .official_fonts
            .len()
            .checked_mul(6)
            .ok_or_else(pdf_size_error)?;
        let first_image_object = after_fallback_forms
            .checked_add(fallback_font_object_count)
            .and_then(|object| object.checked_add(official_font_object_count))
            .ok_or_else(pdf_size_error)?;
        let image_object_count = image_resources
            .len()
            .checked_mul(2)
            .ok_or_else(pdf_size_error)?;
        let first_svg_object = first_image_object
            .checked_add(image_object_count)
            .ok_or_else(pdf_size_error)?;
        let svg_object_count = raw_svg_chunks
            .iter()
            .try_fold(0_usize, |count, chunk| {
                count.checked_add(chunk.chunk.refs().len())
            })
            .ok_or_else(pdf_size_error)?;
        let svg_chunks = renumber_svg_chunks(raw_svg_chunks, first_svg_object)?;
        let object_count = object_count
            .checked_add(fallback_font_object_count)
            .and_then(|count| count.checked_add(official_font_object_count))
            .and_then(|count| count.checked_add(image_object_count))
            .and_then(|count| count.checked_add(svg_object_count))
            .ok_or_else(pdf_size_error)?;
        let page_width = format_pdf_number(self.page_width, "PDF page width")?;
        let page_height = format_pdf_number(self.page_height, "PDF page height")?;

        let output_capacity = 2_048_usize
            .checked_add(content.len())
            .and_then(|capacity| capacity.checked_add(srgb_icc_profile.len()))
            .and_then(|capacity| {
                opacity_resources
                    .len()
                    .checked_mul(256)
                    .and_then(|extra| capacity.checked_add(extra))
            })
            .and_then(|capacity| {
                svg_chunks.iter().try_fold(capacity, |capacity, resource| {
                    capacity.checked_add(resource.bytes.len())
                })
            })
            .and_then(|capacity| {
                xmp.as_ref().map_or(Some(capacity), |xmp| {
                    xmp.len()
                        .checked_add(128)
                        .and_then(|extra| capacity.checked_add(extra))
                })
            })
            .and_then(|capacity| {
                embedded_scene.map_or(Some(capacity), |scene| {
                    scene
                        .len()
                        .checked_add(1_024)
                        .and_then(|extra| capacity.checked_add(extra))
                })
            })
            .and_then(|capacity| {
                link_count
                    .checked_mul(512)
                    .and_then(|extra| capacity.checked_add(extra))
            })
            .and_then(|capacity| {
                fallback_forms.iter().try_fold(capacity, |capacity, form| {
                    form.len()
                        .checked_add(256)
                        .and_then(|extra| capacity.checked_add(extra))
                })
            })
            .and_then(|capacity| {
                self.fallback_font_sfnt
                    .as_ref()
                    .map_or(Some(capacity), |font| {
                        font.len()
                            .checked_add(16_384)
                            .and_then(|extra| capacity.checked_add(extra))
                    })
            })
            .and_then(|capacity| {
                self.official_fonts
                    .iter()
                    .try_fold(capacity, |capacity, font| {
                        font.sfnt
                            .len()
                            .checked_add(16_384)
                            .and_then(|extra| capacity.checked_add(extra))
                    })
            })
            .and_then(|capacity| {
                image_resources
                    .iter()
                    .try_fold(capacity, |capacity, asset| {
                        usize::try_from(asset.width)
                            .ok()?
                            .checked_mul(usize::try_from(asset.height).ok()?)
                            .and_then(|pixels| pixels.checked_mul(4))
                            .and_then(|bytes| bytes.checked_add(1_024))
                            .and_then(|extra| capacity.checked_add(extra))
                    })
            })
            .and_then(|capacity| {
                object_count
                    .checked_mul(32)
                    .and_then(|extra| capacity.checked_add(extra))
            })
            .ok_or_else(pdf_size_error)?;
        let mut pdf = Vec::new();
        pdf.try_reserve_exact(output_capacity).map_err(|_| {
            FormatEncodeError::MemoryUnavailable {
                subject_kind: "artifact",
                subject: "pdf_output",
            }
        })?;
        pdf.extend_from_slice(b"%PDF-1.7\n%\x80\x80\x80\x80\n\n");
        let mut offsets = Vec::new();
        offsets.try_reserve_exact(object_count).map_err(|_| {
            FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_object_offsets",
            }
        })?;
        offsets.resize(object_count, 0);

        begin_object(&mut pdf, &mut offsets, 1)?;
        write!(
            pdf,
            "<<\n/Type /Catalog\n/Pages 2 0 R\n/OutputIntents [{OUTPUT_INTENT_OBJECT} 0 R]\n"
        )
        .map_err(write_error)?;
        if let Some(metadata_object) = metadata_object {
            writeln!(pdf, "/Metadata {metadata_object} 0 R").map_err(write_error)?;
        }
        if let Some(filespec) = attachment_filespec_object {
            writeln!(
                pdf,
                "/Names << /EmbeddedFiles << /Names [(scene.excalidraw) {filespec} 0 R] >> >>\n/AF [{filespec} 0 R]"
            )
            .map_err(write_error)?;
        }
        pdf.extend_from_slice(b">>\nendobj\n");

        begin_object(&mut pdf, &mut offsets, 2)?;
        pdf.extend_from_slice(b"<<\n/Type /Pages\n/Kids [3 0 R]\n/Count 1\n>>\nendobj\n");

        begin_object(&mut pdf, &mut offsets, 3)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /Page\n/Parent 2 0 R\n",
                "/MediaBox [0 0 {} {}]\n",
                "/CropBox [0 0 {} {}]\n",
                "/Resources <<\n/ColorSpace << /CS1 [/ICCBased 5 0 R] >>\n"
            ),
            page_width, page_height, page_width, page_height,
        )
        .map_err(write_error)?;
        if !opacity_resources.is_empty() {
            pdf.extend_from_slice(b"/ExtGState <<");
            for index in 0..opacity_resources.len() {
                write!(
                    pdf,
                    " /GS{} {} 0 R",
                    index + 1,
                    checked_object(FIRST_EXT_GSTATE_OBJECT, index, 1)?
                )
                .map_err(write_error)?;
            }
            pdf.extend_from_slice(b" >>\n");
        }
        if !fallback_forms.is_empty() || !image_resources.is_empty() || !svg_chunks.is_empty() {
            pdf.extend_from_slice(b"/XObject <<");
            for index in 0..fallback_forms.len() {
                write!(
                    pdf,
                    " /FB{} {} 0 R",
                    index + 1,
                    checked_object(first_fallback_form_object, index, 1)?
                )
                .map_err(write_error)?;
            }
            for index in 0..image_resources.len() {
                write!(
                    pdf,
                    " /Im{} {} 0 R",
                    index,
                    checked_object(first_image_object, index, 2)?
                )
                .map_err(write_error)?;
            }
            for (index, resource) in svg_chunks.iter().enumerate() {
                write!(pdf, " /Svg{index} {} 0 R", resource.root.get()).map_err(write_error)?;
            }
            pdf.extend_from_slice(b" >>\n");
        }
        if let Some(fallback_font_object) = fallback_font_object {
            write!(pdf, "/Font << /F1 {fallback_font_object} 0 R").map_err(write_error)?;
            if let Some(official_font_object) = official_font_object {
                for index in 0..self.official_fonts.len() {
                    write!(
                        pdf,
                        " /F{} {} 0 R",
                        index + 2,
                        checked_object(official_font_object, index, 6)?
                    )
                    .map_err(write_error)?;
                }
            }
            pdf.extend_from_slice(b" >>\n");
        } else if let Some(official_font_object) = official_font_object {
            pdf.extend_from_slice(b"/Font <<");
            for index in 0..self.official_fonts.len() {
                write!(
                    pdf,
                    " /F{} {} 0 R",
                    index + 2,
                    checked_object(official_font_object, index, 6)?
                )
                .map_err(write_error)?;
            }
            pdf.extend_from_slice(b" >>\n");
        }
        pdf.extend_from_slice(b">>\n");
        if link_count != 0 {
            pdf.extend_from_slice(b"/Annots [");
            for index in 0..link_count {
                write!(pdf, " {} 0 R", checked_object(first_link_object, index, 1)?)
                    .map_err(write_error)?;
            }
            pdf.extend_from_slice(b" ]\n");
        }
        pdf.extend_from_slice(b"/Contents 4 0 R\n>>\nendobj\n");

        begin_object(&mut pdf, &mut offsets, 4)?;
        write!(pdf, "<<\n/Length {}\n>>\nstream\n", content.len()).map_err(write_error)?;
        pdf.extend_from_slice(content.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        begin_object(&mut pdf, &mut offsets, ICC_PROFILE_OBJECT)?;
        write!(
            pdf,
            "<<\n/N 3\n/Alternate /DeviceRGB\n/Length {}\n>>\nstream\n",
            srgb_icc_profile.len()
        )
        .map_err(write_error)?;
        pdf.extend_from_slice(&srgb_icc_profile);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        begin_object(&mut pdf, &mut offsets, OUTPUT_INTENT_OBJECT)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /OutputIntent\n/S /GTS_PDFA1\n",
                "/OutputConditionIdentifier (sRGB)\n",
                "/Info (excalidraw-render project sRGB)\n",
                "/DestOutputProfile 5 0 R\n>>\nendobj\n"
            )
        )
        .map_err(write_error)?;

        for (index, opacity) in opacity_resources.iter().copied().enumerate() {
            let opacity = format_pdf_number(opacity, "PDF opacity")?;
            begin_object(
                &mut pdf,
                &mut offsets,
                checked_object(FIRST_EXT_GSTATE_OBJECT, index, 1)?,
            )?;
            write!(
                pdf,
                "<<\n/Type /ExtGState\n/CA {opacity}\n/ca {opacity}\n/BM /Normal\n>>\nendobj\n"
            )
            .map_err(write_error)?;
        }
        if let (Some(metadata_object), Some(xmp)) = (metadata_object, xmp.as_ref()) {
            begin_object(&mut pdf, &mut offsets, metadata_object)?;
            write!(
                pdf,
                "<<\n/Type /Metadata\n/Subtype /XML\n/Length {}\n>>\nstream\n",
                xmp.len()
            )
            .map_err(write_error)?;
            pdf.extend_from_slice(xmp.as_bytes());
            pdf.extend_from_slice(b"\nendstream\nendobj\n");
        }
        if let (Some(scene), Some(filespec)) = (embedded_scene, attachment_filespec_object) {
            begin_object(&mut pdf, &mut offsets, first_attachment_object)?;
            write!(
                pdf,
                "<<\n/Type /EmbeddedFile\n/Subtype /application#2Fvnd.excalidraw+json\n/Length {}\n>>\nstream\n",
                scene.len()
            )
            .map_err(write_error)?;
            pdf.extend_from_slice(scene);
            pdf.extend_from_slice(b"\nendstream\nendobj\n");

            begin_object(&mut pdf, &mut offsets, filespec)?;
            write!(
                pdf,
                concat!(
                    "<<\n/Type /Filespec\n/F (scene.excalidraw)\n/UF (scene.excalidraw)\n",
                    "/AFRelationship /Data\n/Desc (Canonical Excalidraw scene)\n",
                    "/EF << /F {} 0 R /UF {} 0 R >>\n>>\nendobj\n"
                ),
                first_attachment_object, first_attachment_object
            )
            .map_err(write_error)?;
        }
        let mut link_index = 0_usize;
        for element in self.elements.iter().rev() {
            let Some((link, width, height, transform, frame_clips)) = element.link_geometry()
            else {
                continue;
            };
            if !link_annotation_visible(
                width,
                height,
                transform,
                frame_clips,
                self.scale * POINTS_PER_LOGICAL_PIXEL,
                self.page_width,
                self.page_height,
            )? {
                continue;
            }
            let object = first_link_object
                .checked_add(link_index)
                .ok_or_else(pdf_size_error)?;
            begin_object(&mut pdf, &mut offsets, object)?;
            self.write_link_annotation(&mut pdf, link, width, height, transform, frame_clips)?;
            link_index = link_index.saturating_add(1);
        }
        for (index, form) in fallback_forms.iter().enumerate() {
            let object = first_fallback_form_object
                .checked_add(index)
                .ok_or_else(pdf_size_error)?;
            let fallback = self
                .elements
                .iter()
                .filter_map(|element| match element {
                    PdfElement::Fallback {
                        width,
                        height,
                        opacity,
                        ..
                    } if *opacity != 1.0 => Some((*width, *height)),
                    _ => None,
                })
                .nth(index)
                .ok_or_else(pdf_size_error)?;
            begin_object(&mut pdf, &mut offsets, object)?;
            pdf.extend_from_slice(b"<<\n/Type /XObject\n/Subtype /Form\n/FormType 1\n/BBox [0 0 ");
            push_pdf_vec_number(&mut pdf, fallback.0, "PDF fallback form width")?;
            pdf.push(b' ');
            push_pdf_vec_number(&mut pdf, fallback.1, "PDF fallback form height")?;
            pdf.extend_from_slice(b"]\n/Resources << /ColorSpace << /CS1 [/ICCBased 5 0 R] >>");
            if let Some(font_object) = fallback_font_object {
                write!(pdf, " /Font << /F1 {font_object} 0 R >>").map_err(write_error)?;
            }
            write!(
                pdf,
                concat!(
                    " >>\n/Group << /S /Transparency /I true /CS /DeviceRGB >>\n",
                    "/Length {}\n>>\nstream\n"
                ),
                form.len()
            )
            .map_err(write_error)?;
            pdf.extend_from_slice(form.as_bytes());
            pdf.extend_from_slice(b"\nendstream\nendobj\n");
        }
        if let (Some(font_object), Some(font)) =
            (fallback_font_object, self.fallback_font_sfnt.as_deref())
        {
            self.write_font_objects(
                &mut pdf,
                &mut offsets,
                font_object,
                font,
                "LiberationSans",
                &fallback_mappings,
            )?;
        }
        if let Some(first_object) = official_font_object {
            for (index, font) in self.official_fonts.iter().enumerate() {
                self.write_font_objects(
                    &mut pdf,
                    &mut offsets,
                    checked_object(first_object, index, 6)?,
                    &font.sfnt,
                    font.pdf_name,
                    &official_mappings[index],
                )?;
            }
        }
        for (index, asset) in image_resources.iter().enumerate() {
            let image_object = checked_object(first_image_object, index, 2)?;
            let mask_object = image_object.checked_add(1).ok_or_else(pdf_size_error)?;
            let pixel_count = usize::try_from(asset.width)
                .ok()
                .and_then(|width| width.checked_mul(usize::try_from(asset.height).ok()?))
                .ok_or_else(pdf_size_error)?;
            let mut rgb = Vec::new();
            let mut alpha = Vec::new();
            let rgb_bytes = pixel_count.checked_mul(3).ok_or_else(pdf_size_error)?;
            rgb.try_reserve_exact(rgb_bytes)
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_resource",
                    subject: "pdf_image_rgb",
                })?;
            alpha.try_reserve_exact(pixel_count).map_err(|_| {
                FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_resource",
                    subject: "pdf_image_alpha",
                }
            })?;
            let rgba = asset.rgba.as_deref().ok_or_else(|| {
                FormatEncodeError::Encoding("PDF image has no normalized pixel resource".to_owned())
            })?;
            for pixel in rgba.chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
                alpha.push(pixel[3]);
            }
            begin_object(&mut pdf, &mut offsets, image_object)?;
            write!(
                pdf,
                concat!(
                    "<<\n/Type /XObject\n/Subtype /Image\n/Width {}\n/Height {}\n",
                    "/ColorSpace /CS1\n/BitsPerComponent 8\n/SMask {} 0 R\n/Length {}\n",
                    ">>\nstream\n"
                ),
                asset.width,
                asset.height,
                mask_object,
                rgb.len()
            )
            .map_err(write_error)?;
            pdf.extend_from_slice(&rgb);
            pdf.extend_from_slice(b"\nendstream\nendobj\n");
            begin_object(&mut pdf, &mut offsets, mask_object)?;
            write!(
                pdf,
                concat!(
                    "<<\n/Type /XObject\n/Subtype /Image\n/Width {}\n/Height {}\n",
                    "/ColorSpace /DeviceGray\n/BitsPerComponent 8\n/Length {}\n>>\nstream\n"
                ),
                asset.width,
                asset.height,
                alpha.len()
            )
            .map_err(write_error)?;
            pdf.extend_from_slice(&alpha);
            pdf.extend_from_slice(b"\nendstream\nendobj\n");
        }
        for resource in &svg_chunks {
            append_chunk(&mut pdf, &mut offsets, resource)?;
        }

        let xref_offset = pdf.len();
        write!(pdf, "xref\n0 {}\n0000000000 65535 f\r\n", object_count + 1).map_err(write_error)?;
        for offset in &offsets {
            writeln!(pdf, "{offset:010} 00000 n\r").map_err(write_error)?;
        }
        write!(
            pdf,
            "trailer\n<<\n/Size {}\n/Root 1 0 R\n>>\nstartxref\n{xref_offset}\n%%EOF",
            object_count + 1
        )
        .map_err(write_error)?;
        Ok(pdf)
    }

    pub(super) fn source_count(&self) -> usize {
        self.elements.len()
    }

    pub(super) fn source(&self, index: usize) -> Option<(Option<&str>, &str)> {
        self.elements.get(index).map(|element| match element {
            PdfElement::RoughOutline { source, .. } => (Some(source.id()), source.pointer()),
            PdfElement::Freedraw { source, .. } => (Some(source.id()), source.pointer()),
            PdfElement::Frame { source, .. } => (Some(source.id()), source.pointer()),
            PdfElement::Text { source, .. } => (Some(source.id()), source.pointer()),
            PdfElement::Image { source, .. } => (Some(source.id()), source.pointer()),
            PdfElement::Fallback { source, .. } => (source.id.as_deref(), source.pointer.as_str()),
        })
    }

    fn write_link_annotation(
        &self,
        pdf: &mut Vec<u8>,
        link: &str,
        width: f64,
        height: f64,
        transform: &super::ResolvedTransform,
        frame_clips: &[super::ResolvedClip],
    ) -> Result<(), FormatEncodeError> {
        let point_scale = self.scale * POINTS_PER_LOGICAL_PIXEL;
        let points = clipped_link_polygon(
            width,
            height,
            transform,
            frame_clips,
            point_scale,
            self.page_width,
            self.page_height,
        )?;
        if points.len() < 3 {
            return Err(FormatEncodeError::Encoding(
                "PDF link annotation has no clipped area".to_owned(),
            ));
        }
        let min_x = points
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min);
        let min_y = points
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = points
            .iter()
            .map(|point| point.1)
            .fold(f64::NEG_INFINITY, f64::max);

        pdf.extend_from_slice(b"<<\n/Type /Annot\n/Subtype /Link\n/Rect [");
        push_pdf_vec_number(pdf, min_x, "PDF link rectangle x minimum")?;
        pdf.push(b' ');
        push_pdf_vec_number(pdf, min_y, "PDF link rectangle y minimum")?;
        pdf.push(b' ');
        push_pdf_vec_number(pdf, max_x, "PDF link rectangle x maximum")?;
        pdf.push(b' ');
        push_pdf_vec_number(pdf, max_y, "PDF link rectangle y maximum")?;
        pdf.extend_from_slice(b"]\n/QuadPoints [");
        let mut first = true;
        let mut write_quadrilateral = |quadrilateral: [(f64, f64); 4]| {
            for (x, y) in quadrilateral {
                if !first {
                    pdf.push(b' ');
                }
                first = false;
                push_pdf_vec_number(pdf, x, "PDF link quadrilateral x")?;
                pdf.push(b' ');
                push_pdf_vec_number(pdf, y, "PDF link quadrilateral y")?;
            }
            Ok::<(), FormatEncodeError>(())
        };
        if points.len() == 4 {
            write_quadrilateral([points[0], points[1], points[3], points[2]])?;
        } else {
            for index in 1..points.len() - 1 {
                write_quadrilateral([
                    points[0],
                    points[index],
                    points[index + 1],
                    points[index + 1],
                ])?;
            }
        }
        pdf.extend_from_slice(b"]\n/Border [0 0 0]\n/A <<\n/S /URI\n/URI (");
        push_pdf_link(pdf, link);
        pdf.extend_from_slice(b")\n>>\n>>\nendobj\n");
        Ok(())
    }

    fn write_font_objects(
        &self,
        pdf: &mut Vec<u8>,
        offsets: &mut [usize],
        font_object: usize,
        font: &[u8],
        base_font: &str,
        mappings: &[PdfGlyphMapping],
    ) -> Result<(), FormatEncodeError> {
        let cid_font_object = font_object.checked_add(1).ok_or_else(pdf_size_error)?;
        let descriptor_object = font_object.checked_add(2).ok_or_else(pdf_size_error)?;
        let font_file_object = font_object.checked_add(3).ok_or_else(pdf_size_error)?;
        let to_unicode_object = font_object.checked_add(4).ok_or_else(pdf_size_error)?;
        let cid_to_gid_object = font_object.checked_add(5).ok_or_else(pdf_size_error)?;
        let face = rustybuzz::ttf_parser::Face::parse(font, 0).map_err(|_| {
            FormatEncodeError::Encoding("PDF fallback font is not a valid sfnt".to_owned())
        })?;
        let units_per_em = i64::from(face.units_per_em());
        let metric = |value: i16| i64::from(value) * 1_000 / units_per_em;
        let bbox = face.global_bounding_box();

        begin_object(pdf, offsets, font_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /Font\n/Subtype /Type0\n/BaseFont /{}\n",
                "/Encoding /Identity-H\n/DescendantFonts [{} 0 R]\n",
                "/ToUnicode {} 0 R\n>>\nendobj\n"
            ),
            base_font, cid_font_object, to_unicode_object,
        )
        .map_err(write_error)?;

        begin_object(pdf, offsets, cid_font_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /Font\n/Subtype /CIDFontType2\n/BaseFont /{}\n",
                "/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>\n",
                "/FontDescriptor {} 0 R\n/CIDToGIDMap {} 0 R\n/DW 1000\n/W ["
            ),
            base_font, descriptor_object, cid_to_gid_object,
        )
        .map_err(write_error)?;
        for mapping in mappings {
            write!(pdf, " {} [{}]", mapping.code, mapping.advance_1000).map_err(write_error)?;
        }
        pdf.extend_from_slice(b" ]\n>>\nendobj\n");

        begin_object(pdf, offsets, descriptor_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /FontDescriptor\n/FontName /{}\n/Flags 32\n",
                "/FontBBox [{} {} {} {}]\n/ItalicAngle 0\n/Ascent {}\n/Descent {}\n",
                "/CapHeight {}\n/StemV 80\n/FontFile2 {} 0 R\n>>\nendobj\n"
            ),
            base_font,
            i64::from(bbox.x_min) * 1_000 / units_per_em,
            i64::from(bbox.y_min) * 1_000 / units_per_em,
            i64::from(bbox.x_max) * 1_000 / units_per_em,
            i64::from(bbox.y_max) * 1_000 / units_per_em,
            metric(face.ascender()),
            metric(face.descender()),
            metric(face.capital_height().unwrap_or(face.ascender())),
            font_file_object,
        )
        .map_err(write_error)?;

        begin_object(pdf, offsets, font_file_object)?;
        write!(
            pdf,
            "<<\n/Length {}\n/Length1 {}\n>>\nstream\n",
            font.len(),
            font.len()
        )
        .map_err(write_error)?;
        pdf.extend_from_slice(font);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        let cmap = to_unicode_cmap(mappings, base_font)?;
        begin_object(pdf, offsets, to_unicode_object)?;
        write!(pdf, "<<\n/Length {}\n>>\nstream\n", cmap.len()).map_err(write_error)?;
        pdf.extend_from_slice(cmap.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        let map_length = mappings.last().map_or(2_usize, |mapping| {
            usize::from(mapping.code)
                .saturating_add(1)
                .saturating_mul(2)
        });
        let mut cid_to_gid = Vec::new();
        cid_to_gid.try_reserve_exact(map_length).map_err(|_| {
            FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_cid_to_gid_map",
            }
        })?;
        cid_to_gid.resize(map_length, 0);
        for mapping in mappings {
            let offset = usize::from(mapping.code).saturating_mul(2);
            let [high, low] = mapping.glyph_id.to_be_bytes();
            cid_to_gid[offset] = high;
            cid_to_gid[offset + 1] = low;
        }
        begin_object(pdf, offsets, cid_to_gid_object)?;
        write!(pdf, "<<\n/Length {}\n>>\nstream\n", cid_to_gid.len()).map_err(write_error)?;
        pdf.extend_from_slice(&cid_to_gid);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        Ok(())
    }

    fn fallback_glyph_mappings(&self) -> Result<Vec<PdfGlyphMapping>, FormatEncodeError> {
        glyph_mappings(
            self.elements.iter().filter_map(|element| match element {
                PdfElement::Fallback {
                    label: Some(label), ..
                } => Some(label),
                PdfElement::Frame {
                    name: Some(label), ..
                } => Some(label),
                _ => None,
            }),
            "pdf_fallback_glyph_mappings",
        )
    }

    fn official_glyph_mappings(&self) -> Result<Vec<Vec<PdfGlyphMapping>>, FormatEncodeError> {
        let mut all_mappings = Vec::new();
        all_mappings
            .try_reserve_exact(self.official_fonts.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_text_font_mappings",
            })?;
        for font_index in 0..self.official_fonts.len() {
            all_mappings.push(glyph_mappings(
                self.elements.iter().flat_map(|element| match element {
                    PdfElement::Text { text, .. } if text.font_index == font_index => {
                        text.lines.as_slice()
                    }
                    PdfElement::RoughOutline {
                        generated_label: Some(text),
                        ..
                    } if text.font_index == font_index => text.lines.as_slice(),
                    _ => &[],
                }),
                "pdf_text_glyph_mappings",
            )?);
        }
        Ok(all_mappings)
    }

    fn opacity_resources(&self) -> Result<Vec<f64>, FormatEncodeError> {
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(self.elements.len().saturating_add(1))
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_opacity_resources",
            })?;
        if let Some(background) = self.background.filter(|color| color.alpha() != 1.0) {
            resources.push(background.alpha());
        }
        for element in &self.elements {
            let mut retain = |opacity: f64| {
                if opacity != 1.0
                    && !resources
                        .iter()
                        .any(|retained: &f64| retained.to_bits() == opacity.to_bits())
                {
                    resources.push(opacity);
                }
            };
            match element {
                PdfElement::RoughOutline {
                    opacity,
                    stroke_color,
                    fill_color,
                    drawing,
                    background_color,
                    generated_label_color,
                    ..
                } => {
                    retain(*opacity * stroke_color.alpha());
                    if let Some(fill_color) = fill_color {
                        retain(*opacity * fill_color.alpha());
                    }
                    for decoration in &drawing.decorations {
                        if decoration.drawing.fill.is_some() {
                            let color = super::decoration_fill_color(
                                decoration,
                                *stroke_color,
                                *background_color,
                            );
                            retain(*opacity * color.alpha());
                        }
                    }
                    if let Some(color) = generated_label_color {
                        retain(color.alpha());
                    }
                }
                PdfElement::Freedraw {
                    color,
                    background_color,
                    opacity,
                    ..
                } => {
                    retain(*opacity * color.alpha());
                    if let Some(background_color) = background_color {
                        retain(*opacity * background_color.alpha());
                    }
                }
                PdfElement::Frame { opacity, .. } => retain(*opacity),
                PdfElement::Text { color, opacity, .. } => retain(*opacity * color.alpha()),
                PdfElement::Image { opacity, .. } => retain(*opacity),
                PdfElement::Fallback { opacity, .. } => retain(*opacity),
            }
        }
        Ok(resources)
    }

    fn encode_fallback_forms(
        &self,
        fallback_mappings: &[PdfGlyphMapping],
    ) -> Result<Vec<String>, FormatEncodeError> {
        let form_count = self
            .elements
            .iter()
            .filter(|element| {
                matches!(element, PdfElement::Fallback { opacity, .. } if *opacity != 1.0)
            })
            .count();
        let mut forms = Vec::new();
        forms
            .try_reserve_exact(form_count)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_fallback_forms",
            })?;
        for element in &self.elements {
            let PdfElement::Fallback {
                label,
                width,
                height,
                opacity,
                ..
            } = element
            else {
                continue;
            };
            if *opacity == 1.0 {
                continue;
            }
            let mut form = String::new();
            let capacity = label.as_ref().map_or(1_024, |label| {
                label
                    .path
                    .operations
                    .len()
                    .saturating_mul(160)
                    .saturating_add(label.text.len().saturating_mul(4))
                    .saturating_add(1_024)
            });
            form.try_reserve_exact(capacity)
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_workspace",
                    subject: "pdf_fallback_form_stream",
                })?;
            push_fallback_body(
                &mut form,
                *width,
                *height,
                label.as_ref(),
                self.fallback_colors,
                fallback_mappings,
            )?;
            forms.push(form);
        }
        Ok(forms)
    }

    fn image_resources(
        &self,
    ) -> Result<Vec<std::sync::Arc<super::ResolvedImageAsset>>, FormatEncodeError> {
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(self.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_image_resources",
            })?;
        for element in &self.elements {
            let PdfElement::Image { asset, .. } = element else {
                continue;
            };
            if asset.sanitized_svg.is_some() {
                continue;
            }
            if resources
                .iter()
                .any(|candidate: &std::sync::Arc<super::ResolvedImageAsset>| {
                    candidate.identity == asset.identity
                })
            {
                continue;
            }
            resources.push(std::sync::Arc::clone(asset));
        }
        resources.sort_unstable_by_key(|asset| asset.identity);
        Ok(resources)
    }

    fn svg_resources(&self) -> Result<Vec<PdfSvgResource>, FormatEncodeError> {
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(self.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_svg_resources",
            })?;
        for element in &self.elements {
            let PdfElement::Image { asset, .. } = element else {
                continue;
            };
            if asset.sanitized_svg.is_none() {
                continue;
            }
            let has_effects = !asset.svg_effects.is_empty();
            if resources.iter().any(|candidate: &PdfSvgResource| {
                if has_effects {
                    std::sync::Arc::ptr_eq(&candidate.asset, asset)
                } else {
                    candidate.asset.identity == asset.identity
                }
            }) {
                continue;
            }
            let raster_scale = if has_effects {
                (width_scale(element) * self.scale).max(1.0)
            } else {
                1.0
            };
            resources.push(PdfSvgResource {
                asset: std::sync::Arc::clone(asset),
                raster_scale,
            });
        }
        resources.sort_by_key(|resource| resource.asset.identity);
        Ok(resources)
    }

    fn encode_content(
        &self,
        opacity_resources: &[f64],
        image_resources: &[std::sync::Arc<super::ResolvedImageAsset>],
        svg_resources: &[PdfSvgResource],
        fallback_mappings: &[PdfGlyphMapping],
        official_mappings: &[Vec<PdfGlyphMapping>],
    ) -> Result<String, FormatEncodeError> {
        let mut capacity = 1_024_usize;
        for element in &self.elements {
            let element_bytes = match element {
                PdfElement::RoughOutline {
                    drawing,
                    generated_label,
                    ..
                } => drawing
                    .operation_count()
                    .checked_mul(200)
                    .and_then(|path_bytes| {
                        path_bytes.checked_add(generated_label.as_ref().map_or(512, |label| {
                            label.lines.iter().fold(1_024_usize, |size, line| {
                                size.saturating_add(line.glyphs.len().saturating_mul(96))
                            })
                        }))
                    })
                    .ok_or_else(pdf_size_error)?,
                PdfElement::Freedraw {
                    path,
                    background_fill,
                    ..
                } => path
                    .operation_count()
                    .saturating_add(
                        background_fill
                            .as_ref()
                            .map_or(0, |fill| fill.path.operations.len()),
                    )
                    .checked_mul(160)
                    .and_then(|path_bytes| path_bytes.checked_add(256))
                    .ok_or_else(pdf_size_error)?,
                PdfElement::Frame { name, .. } => name
                    .as_ref()
                    .map_or(512, |name| name.text.len().saturating_add(1_024)),
                PdfElement::Text { text, .. } => text.lines.iter().fold(512_usize, |size, line| {
                    size.saturating_add(line.text.len().saturating_mul(4))
                        .saturating_add(line.glyphs.len().saturating_mul(96))
                }),
                PdfElement::Image { .. } => 512,
                PdfElement::Fallback { label, .. } => label.as_ref().map_or(1_024, |label| {
                    label
                        .path
                        .operations
                        .len()
                        .saturating_mul(160)
                        .saturating_add(label.text.len().saturating_mul(4))
                        .saturating_add(1_024)
                }),
            };
            capacity = capacity
                .checked_add(element_bytes)
                .ok_or_else(pdf_size_error)?;
        }
        let mut content = String::new();
        content
            .try_reserve_exact(capacity)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_content_stream",
            })?;
        content.push_str("q\n");
        push_matrix(
            &mut content,
            self.scale * POINTS_PER_LOGICAL_PIXEL,
            0.0,
            0.0,
            -self.scale * POINTS_PER_LOGICAL_PIXEL,
            0.0,
            self.page_height,
        )?;
        if let Some(background) = self.background {
            if background.alpha() != 1.0 {
                content.push_str("q\n");
                push_opacity_resource(&mut content, opacity_resources, background.alpha())?;
            }
            content.push_str("/CS1 cs\n");
            push_color(&mut content, background.rgb_f64(), "scn")?;
            push_pdf_number(&mut content, 0.0, "PDF background x")?;
            content.push(' ');
            push_pdf_number(&mut content, 0.0, "PDF background y")?;
            content.push(' ');
            push_pdf_number(&mut content, self.logical_width, "PDF background width")?;
            content.push(' ');
            push_pdf_number(&mut content, self.logical_height, "PDF background height")?;
            content.push_str(" re\nf\n");
            if background.alpha() != 1.0 {
                content.push_str("Q\n");
            }
        }
        let mut fallback_form_index = 0_usize;
        for element in &self.elements {
            match element {
                PdfElement::RoughOutline {
                    source,
                    drawing,
                    frame_clips,
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
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF plan outline lost its source identity".to_owned(),
                        ));
                    }
                    let (sin, cos) = if transform.angle_radians == 0.0 {
                        (0.0, 1.0)
                    } else {
                        (transform.angle_radians.sin(), transform.angle_radians.cos())
                    };
                    let offset_x = transform.translation_x + transform.center_x
                        - cos * transform.center_x
                        + sin * transform.center_y;
                    let offset_y = transform.translation_y + transform.center_y
                        - sin * transform.center_x
                        - cos * transform.center_y;
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    if let Some(mask) = label_mask {
                        content.push_str("0 0 ");
                        push_pdf_number(
                            &mut content,
                            self.logical_width,
                            "PDF arrow mask outer width",
                        )?;
                        content.push(' ');
                        push_pdf_number(
                            &mut content,
                            self.logical_height,
                            "PDF arrow mask outer height",
                        )?;
                        content.push_str(" re\n");
                        push_pdf_number(&mut content, mask.x, "PDF arrow label mask x")?;
                        content.push(' ');
                        push_pdf_number(&mut content, mask.y, "PDF arrow label mask y")?;
                        content.push(' ');
                        push_pdf_number(&mut content, mask.width, "PDF arrow label mask width")?;
                        content.push(' ');
                        push_pdf_number(&mut content, mask.height, "PDF arrow label mask height")?;
                        content.push_str(" re\nW*\nn\n");
                    }
                    push_matrix(&mut content, cos, sin, -sin, cos, offset_x, offset_y)?;
                    if let (Some(fill), Some(fill_color)) = (&drawing.fill, fill_color) {
                        content.push_str("q\n");
                        let fill_opacity = *opacity * fill_color.alpha();
                        if fill_opacity != 1.0 {
                            push_opacity_resource(&mut content, opacity_resources, fill_opacity)?;
                        }
                        match fill.kind {
                            FillKind::Solid => {
                                content.push_str("/CS1 cs\n");
                                push_color(&mut content, fill_color.rgb_f64(), "scn")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str(if drawing.shape.canvas_uses_even_odd_fill() {
                                    "f*\n"
                                } else {
                                    "f\n"
                                });
                            }
                            FillKind::Sketch => {
                                push_pdf_number(
                                    &mut content,
                                    *source_stroke_width / 2.0,
                                    "PDF fill weight",
                                )?;
                                content.push_str(" w\n1 J\n/CS1 CS\n");
                                push_color(&mut content, fill_color.rgb_f64(), "SCN")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str("S\n");
                            }
                        }
                        content.push_str("Q\n");
                    }
                    let stroke_opacity = *opacity * stroke_color.alpha();
                    if stroke_opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, stroke_opacity)?;
                    }
                    push_pdf_number(&mut content, *stroke_width, "PDF stroke width")?;
                    content.push_str(" w\n1 J\n");
                    match stroke_style {
                        StrokeStyle::Solid => {}
                        StrokeStyle::Dashed => {
                            content.push_str("[8 ");
                            push_pdf_number(
                                &mut content,
                                8.0 + source_stroke_width,
                                "PDF dash gap",
                            )?;
                            content.push_str("] 0 d\n");
                        }
                        StrokeStyle::Dotted => {
                            content.push_str("[1.5 ");
                            push_pdf_number(
                                &mut content,
                                6.0 + source_stroke_width,
                                "PDF dot gap",
                            )?;
                            content.push_str("] 0 d\n");
                        }
                    }
                    content.push_str("/CS1 CS\n");
                    push_color(&mut content, stroke_color.rgb_f64(), "SCN")?;
                    push_rough_path(&mut content, &drawing.outline)?;
                    content.push_str("S\n");
                    for decoration in &drawing.decorations {
                        let fill = super::decoration_fill_color(
                            decoration,
                            *stroke_color,
                            *background_color,
                        );
                        let (decoration_style, decoration_stroke_width) =
                            super::decoration_stroke_style(decoration, *source_stroke_width);
                        push_pdf_drawing(
                            &mut content,
                            &decoration.drawing,
                            *stroke_color,
                            Some(fill),
                            *stroke_width,
                            decoration_style,
                            decoration_stroke_width,
                            *opacity,
                            opacity_resources,
                        )?;
                    }
                    if let (Some(label), Some(color)) = (generated_label, generated_label_color) {
                        content.push_str("q\n1 0 0 1 0 ");
                        push_pdf_number(
                            &mut content,
                            *generated_label_offset_y,
                            "PDF embeddable label offset",
                        )?;
                        content.push_str(" cm\n");
                        let effective_opacity = color.alpha();
                        if effective_opacity != 1.0 {
                            push_opacity_resource(
                                &mut content,
                                opacity_resources,
                                effective_opacity,
                            )?;
                        }
                        content.push_str("/CS1 cs\n");
                        push_color(&mut content, color.rgb_f64(), "scn")?;
                        for line in &label.lines {
                            content.push_str("/Span << /ActualText <FEFF");
                            push_utf16be_hex(&mut content, &line.text)?;
                            content.push_str("> >> BDC\nBT\n/F");
                            write!(content, "{} ", label.font_index + 2).map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write PDF embeddable font resource".to_owned(),
                                )
                            })?;
                            push_pdf_number(
                                &mut content,
                                label.font_size,
                                "PDF embeddable font size",
                            )?;
                            content.push_str(" Tf\n");
                            for glyph in &line.glyphs {
                                content.push_str("1 0 0 -1 ");
                                push_pdf_number(&mut content, glyph.x, "PDF embeddable glyph x")?;
                                content.push(' ');
                                push_pdf_number(&mut content, glyph.y, "PDF embeddable glyph y")?;
                                content.push_str(" Tm\n<");
                                let mappings =
                                    official_mappings.get(label.font_index).ok_or_else(|| {
                                        FormatEncodeError::Encoding(format!(
                                            "PDF embeddable references font {} of {} resources",
                                            label.font_index,
                                            official_mappings.len()
                                        ))
                                    })?;
                                let code = glyph_mapping_code(mappings, glyph)?;
                                write!(content, "{code:04X}").map_err(|_| {
                                    FormatEncodeError::Encoding(
                                        "failed to write PDF embeddable glyph ID".to_owned(),
                                    )
                                })?;
                                content.push_str("> Tj\n");
                            }
                            content.push_str("ET\nEMC\n");
                        }
                        content.push_str("Q\n");
                    }
                    content.push_str("Q\n");
                }
                PdfElement::Freedraw {
                    source,
                    path,
                    background_fill,
                    frame_clips,
                    transform,
                    color,
                    background_color,
                    source_stroke_width,
                    opacity,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF freedraw lost its source identity".to_owned(),
                        ));
                    }
                    let (sin, cos) = if transform.angle_radians == 0.0 {
                        (0.0, 1.0)
                    } else {
                        (transform.angle_radians.sin(), transform.angle_radians.cos())
                    };
                    let offset_x = transform.translation_x + transform.center_x
                        - cos * transform.center_x
                        + sin * transform.center_y;
                    let offset_y = transform.translation_y + transform.center_y
                        - sin * transform.center_x
                        - cos * transform.center_y;
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_matrix(&mut content, cos, sin, -sin, cos, offset_x, offset_y)?;
                    if let (Some(fill), Some(background_color)) =
                        (background_fill, background_color)
                    {
                        content.push_str("q\n");
                        let fill_opacity = *opacity * background_color.alpha();
                        if fill_opacity != 1.0 {
                            push_opacity_resource(&mut content, opacity_resources, fill_opacity)?;
                        }
                        match fill.kind {
                            FillKind::Solid => {
                                content.push_str("/CS1 cs\n");
                                push_color(&mut content, background_color.rgb_f64(), "scn")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str("f*\n");
                            }
                            FillKind::Sketch => {
                                push_pdf_number(
                                    &mut content,
                                    *source_stroke_width / 2.0,
                                    "PDF freedraw fill weight",
                                )?;
                                content.push_str(" w\n1 J\n/CS1 CS\n");
                                push_color(&mut content, background_color.rgb_f64(), "SCN")?;
                                push_rough_path(&mut content, &fill.path)?;
                                content.push_str("S\n");
                            }
                        }
                        content.push_str("Q\n");
                    }
                    if path.is_empty() {
                        content.push_str("Q\n");
                        continue;
                    }
                    let effective_opacity = *opacity * color.alpha();
                    if effective_opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, effective_opacity)?;
                    }
                    content.push_str("/CS1 cs\n");
                    push_color(&mut content, color.rgb_f64(), "scn")?;
                    push_freedraw_path(&mut content, path)?;
                    content.push_str("f\nQ\n");
                }
                PdfElement::Frame {
                    source,
                    name,
                    magic,
                    outline_path,
                    frame_clips,
                    transform,
                    opacity,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF frame lost its source identity".to_owned(),
                        ));
                    }
                    let (sin, cos) = if transform.angle_radians == 0.0 {
                        (0.0, 1.0)
                    } else {
                        (transform.angle_radians.sin(), transform.angle_radians.cos())
                    };
                    let offset_x = transform.translation_x + transform.center_x
                        - cos * transform.center_x
                        + sin * transform.center_y;
                    let offset_y = transform.translation_y + transform.center_y
                        - sin * transform.center_x
                        - cos * transform.center_y;
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_matrix(&mut content, cos, sin, -sin, cos, offset_x, offset_y)?;
                    if *opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, *opacity)?;
                    }
                    if let Some(outline_path) = outline_path {
                        content.push_str("2 w\n1 J\n1 j\n/CS1 CS\n0.7333333333333333 0.7333333333333333 0.7333333333333333 SCN\n");
                        push_rough_path(&mut content, outline_path)?;
                        content.push_str("S\n");
                    }
                    if let Some(name) = name {
                        content.push_str("q\n1 0 0 1 0 -24 cm\n/Span << /ActualText <FEFF");
                        push_utf16be_hex(&mut content, &name.text)?;
                        content.push_str("> >> BDC\n");
                        let color = if *magic {
                            CanonicalColor::from_rgb8(0x7a, 0xff, 0xd7)
                        } else {
                            CanonicalColor::from_rgb8(0x1b, 0x1b, 0x1f)
                        };
                        push_pdf_label_text(&mut content, name, color, fallback_mappings)?;
                        content.push_str("EMC\nQ\n");
                    }
                    content.push_str("Q\n");
                }
                PdfElement::Text {
                    source,
                    text,
                    frame_clips,
                    transform,
                    color,
                    opacity,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF text lost its source identity".to_owned(),
                        ));
                    }
                    let (sin, cos) = if transform.angle_radians == 0.0 {
                        (0.0, 1.0)
                    } else {
                        (transform.angle_radians.sin(), transform.angle_radians.cos())
                    };
                    let offset_x = transform.translation_x + transform.center_x
                        - cos * transform.center_x
                        + sin * transform.center_y;
                    let offset_y = transform.translation_y + transform.center_y
                        - sin * transform.center_x
                        - cos * transform.center_y;
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_matrix(&mut content, cos, sin, -sin, cos, offset_x, offset_y)?;
                    let effective_opacity = *opacity * color.alpha();
                    if effective_opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, effective_opacity)?;
                    }
                    content.push_str("/CS1 cs\n");
                    push_color(&mut content, color.rgb_f64(), "scn")?;
                    for line in &text.lines {
                        content.push_str("/Span << /ActualText <FEFF");
                        push_utf16be_hex(&mut content, &line.text)?;
                        content.push_str("> >> BDC\nBT\n/F");
                        write!(content, "{} ", text.font_index + 2).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write PDF text font resource".to_owned(),
                            )
                        })?;
                        push_pdf_number(&mut content, text.font_size, "PDF text font size")?;
                        content.push_str(" Tf\n");
                        for glyph in &line.glyphs {
                            content.push_str("1 0 0 -1 ");
                            push_pdf_number(&mut content, glyph.x, "PDF text glyph x")?;
                            content.push(' ');
                            push_pdf_number(&mut content, glyph.y, "PDF text glyph y")?;
                            content.push_str(" Tm\n<");
                            let mappings =
                                official_mappings.get(text.font_index).ok_or_else(|| {
                                    FormatEncodeError::Encoding(
                                        "PDF text references an unknown font resource".to_owned(),
                                    )
                                })?;
                            let code = glyph_mapping_code(mappings, glyph)?;
                            write!(content, "{code:04X}").map_err(|_| {
                                FormatEncodeError::Encoding(
                                    "failed to write PDF text glyph ID".to_owned(),
                                )
                            })?;
                            content.push_str("> Tj\n");
                        }
                        content.push_str("ET\nEMC\n");
                    }
                    content.push_str("Q\n");
                }
                PdfElement::Image {
                    source,
                    asset,
                    frame_clips,
                    transform,
                    opacity,
                    width,
                    height,
                    source_rect,
                    scale,
                    ..
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF image lost its source identity".to_owned(),
                        ));
                    }
                    let (resource_prefix, resource) = if asset.sanitized_svg.is_some() {
                        (
                            "Svg",
                            svg_resources.iter().position(|candidate| {
                                if asset.svg_effects.is_empty() {
                                    candidate.asset.identity == asset.identity
                                } else {
                                    std::sync::Arc::ptr_eq(&candidate.asset, asset)
                                }
                            }),
                        )
                    } else {
                        (
                            "Im",
                            image_resources
                                .iter()
                                .position(|candidate| candidate.identity == asset.identity),
                        )
                    };
                    let resource = resource.ok_or_else(|| {
                        FormatEncodeError::Encoding(
                            "PDF image resource has no semantic owner".to_owned(),
                        )
                    })?;
                    let (sin, cos) = if transform.angle_radians == 0.0 {
                        (0.0, 1.0)
                    } else {
                        (transform.angle_radians.sin(), transform.angle_radians.cos())
                    };
                    let offset_x = transform.translation_x + transform.center_x
                        - cos * transform.center_x
                        + sin * transform.center_y;
                    let offset_y = transform.translation_y + transform.center_y
                        - sin * transform.center_x
                        - cos * transform.center_y;
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_matrix(&mut content, cos, sin, -sin, cos, offset_x, offset_y)?;
                    if *opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, *opacity)?;
                    }
                    push_pdf_number(&mut content, 0.0, "PDF image clip x")?;
                    content.push(' ');
                    push_pdf_number(&mut content, 0.0, "PDF image clip y")?;
                    content.push(' ');
                    push_pdf_number(&mut content, *width, "PDF image clip width")?;
                    content.push(' ');
                    push_pdf_number(&mut content, *height, "PDF image clip height")?;
                    content.push_str(" re\nW\nn\n");
                    let base_scale_x = *width / source_rect.width;
                    let base_scale_y = *height / source_rect.height;
                    let (image_scale_x, image_offset_x) = if scale[0] < 0 {
                        (
                            -f64::from(asset.width) * base_scale_x,
                            *width + source_rect.x * base_scale_x,
                        )
                    } else {
                        (
                            f64::from(asset.width) * base_scale_x,
                            -source_rect.x * base_scale_x,
                        )
                    };
                    let (image_scale_y, image_offset_y) = if scale[1] < 0 {
                        (
                            -f64::from(asset.height) * base_scale_y,
                            *height + source_rect.y * base_scale_y,
                        )
                    } else {
                        (
                            f64::from(asset.height) * base_scale_y,
                            -source_rect.y * base_scale_y,
                        )
                    };
                    push_matrix(
                        &mut content,
                        image_scale_x,
                        0.0,
                        0.0,
                        image_scale_y,
                        image_offset_x,
                        image_offset_y,
                    )?;
                    writeln!(content, "/{resource_prefix}{resource} Do").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write PDF image invocation".to_owned(),
                        )
                    })?;
                    content.push_str("Q\n");
                }
                PdfElement::Fallback {
                    source,
                    label,
                    link: _,
                    frame_clips,
                    width,
                    height,
                    transform,
                    opacity,
                } => {
                    if source.pointer.is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF fallback lost its source location".to_owned(),
                        ));
                    }
                    let (sin, cos) = if transform.angle_radians == 0.0 {
                        (0.0, 1.0)
                    } else {
                        (transform.angle_radians.sin(), transform.angle_radians.cos())
                    };
                    let offset_x = transform.translation_x + transform.center_x
                        - cos * transform.center_x
                        + sin * transform.center_y;
                    let offset_y = transform.translation_y + transform.center_y
                        - sin * transform.center_x
                        - cos * transform.center_y;
                    content.push_str("q\n");
                    for frame_clip in frame_clips {
                        push_transformed_clip(&mut content, frame_clip)?;
                    }
                    push_matrix(&mut content, cos, sin, -sin, cos, offset_x, offset_y)?;
                    if *opacity != 1.0 {
                        let resource = opacity_resources
                            .iter()
                            .position(|candidate| candidate.to_bits() == opacity.to_bits())
                            .ok_or_else(|| {
                                FormatEncodeError::Encoding(
                                    "PDF fallback opacity resource has no owner".to_owned(),
                                )
                            })?;
                        writeln!(content, "/GS{} gs", resource + 1).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write PDF fallback opacity resource".to_owned(),
                            )
                        })?;
                        fallback_form_index = fallback_form_index.saturating_add(1);
                        writeln!(content, "/FB{fallback_form_index} Do").map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write PDF fallback form invocation".to_owned(),
                            )
                        })?;
                        content.push_str("Q\n");
                        continue;
                    }
                    push_fallback_body(
                        &mut content,
                        *width,
                        *height,
                        label.as_ref(),
                        self.fallback_colors,
                        fallback_mappings,
                    )?;
                    content.push_str("Q\n");
                }
            }
        }
        content.push('Q');
        Ok(content)
    }
}

impl PdfElement {
    fn link_geometry(
        &self,
    ) -> Option<(
        &str,
        f64,
        f64,
        &super::ResolvedTransform,
        &[super::ResolvedClip],
    )> {
        match self {
            Self::RoughOutline {
                link: Some(link),
                width,
                height,
                transform,
                frame_clips,
                ..
            }
            | Self::Freedraw {
                link: Some(link),
                width,
                height,
                transform,
                frame_clips,
                ..
            }
            | Self::Frame {
                link: Some(link),
                width,
                height,
                transform,
                frame_clips,
                ..
            }
            | Self::Text {
                link: Some(link),
                width,
                height,
                transform,
                frame_clips,
                ..
            }
            | Self::Image {
                link: Some(link),
                width,
                height,
                transform,
                frame_clips,
                ..
            }
            | Self::Fallback {
                link: Some(link),
                width,
                height,
                transform,
                frame_clips,
                ..
            } => Some((link, *width, *height, transform, frame_clips)),
            _ => None,
        }
    }
}

fn glyph_mappings<'a>(
    labels: impl IntoIterator<Item = &'a super::text::ResolvedLabel>,
    subject: &'static str,
) -> Result<Vec<PdfGlyphMapping>, FormatEncodeError> {
    let mut mappings = Vec::new();
    for label in labels {
        for glyph in &label.glyphs {
            if mappings.iter().any(|mapping: &PdfGlyphMapping| {
                mapping.glyph_id == glyph.id && mapping.unicode == glyph.unicode
            }) {
                continue;
            }
            let code = u16::try_from(mappings.len().saturating_add(1)).map_err(|_| {
                FormatEncodeError::Encoding(
                    "PDF font requires more deterministic character codes than Identity-H supports"
                        .to_owned(),
                )
            })?;
            let mut unicode = Vec::new();
            unicode
                .try_reserve_exact(glyph.unicode.len())
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_workspace",
                    subject,
                })?;
            unicode.extend_from_slice(&glyph.unicode);
            mappings
                .try_reserve(1)
                .map_err(|_| FormatEncodeError::MemoryUnavailable {
                    subject_kind: "format_workspace",
                    subject,
                })?;
            mappings.push(PdfGlyphMapping {
                code,
                glyph_id: glyph.id,
                advance_1000: glyph.advance_1000,
                unicode,
            });
        }
    }
    Ok(mappings)
}

pub(super) fn page_dimensions(
    scene: &ResolvedScene,
    request: &RenderRequest,
) -> Result<(f64, f64), FormatEncodeError> {
    let scale = scene.raster_or_pdf_scale(request.sizing());
    Ok((
        point_dimension(scene.logical_width * scale)?,
        point_dimension(scene.logical_height * scale)?,
    ))
}

fn transformed_pdf_points(
    width: f64,
    height: f64,
    transform: &super::ResolvedTransform,
    point_scale: f64,
    page_height: f64,
) -> [(f64, f64); 4] {
    let sin = transform.angle_radians.sin();
    let cos = transform.angle_radians.cos();
    let offset_x = transform.translation_x + transform.center_x - cos * transform.center_x
        + sin * transform.center_y;
    let offset_y = transform.translation_y + transform.center_y
        - sin * transform.center_x
        - cos * transform.center_y;
    let map = |x: f64, y: f64| {
        let logical_x = cos * x - sin * y + offset_x;
        let logical_y = sin * x + cos * y + offset_y;
        (
            logical_x * point_scale,
            page_height - logical_y * point_scale,
        )
    };
    [
        map(0.0, 0.0),
        map(width, 0.0),
        map(0.0, height),
        map(width, height),
    ]
}

fn link_annotation_visible(
    width: f64,
    height: f64,
    transform: &super::ResolvedTransform,
    frame_clips: &[super::ResolvedClip],
    point_scale: f64,
    page_width: f64,
    page_height: f64,
) -> Result<bool, FormatEncodeError> {
    Ok(clipped_link_polygon(
        width,
        height,
        transform,
        frame_clips,
        point_scale,
        page_width,
        page_height,
    )?
    .len()
        >= 3)
}

fn clipped_link_polygon(
    width: f64,
    height: f64,
    transform: &super::ResolvedTransform,
    frame_clips: &[super::ResolvedClip],
    point_scale: f64,
    page_width: f64,
    page_height: f64,
) -> Result<Vec<(f64, f64)>, FormatEncodeError> {
    let element = transformed_pdf_points(width, height, transform, point_scale, page_height);
    let mut polygon = Vec::new();
    polygon
        .try_reserve_exact(4)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_link_polygon",
        })?;
    polygon.extend([element[0], element[1], element[3], element[2]]);
    polygon = clip_convex_polygon(
        &polygon,
        &[
            (0.0, 0.0),
            (0.0, page_height),
            (page_width, page_height),
            (page_width, 0.0),
        ],
    )?;
    for frame_clip in frame_clips {
        let frame = transformed_clip_path(frame_clip, point_scale, page_height)?;
        polygon = clip_convex_polygon(&polygon, &frame)?;
    }
    Ok(polygon)
}

fn transformed_clip_path(
    clip: &super::ResolvedClip,
    point_scale: f64,
    page_height: f64,
) -> Result<Vec<(f64, f64)>, FormatEncodeError> {
    const CURVE_STEPS: usize = 8;
    let mut points = Vec::new();
    points
        .try_reserve_exact(clip.path.operations.len().saturating_mul(CURVE_STEPS))
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_link_frame_clip",
        })?;
    let mut current = rough::Point { x: 0.0, y: 0.0 };
    for operation in &clip.path.operations {
        match operation {
            PathOperation::Move(point) => {
                current = *point;
                points.push(transformed_clip_point(
                    current,
                    &clip.transform,
                    point_scale,
                    page_height,
                ));
            }
            PathOperation::Line(point) => {
                current = *point;
                points.push(transformed_clip_point(
                    current,
                    &clip.transform,
                    point_scale,
                    page_height,
                ));
            }
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                let start = current;
                for step in 1..=CURVE_STEPS {
                    let t = step as f64 / CURVE_STEPS as f64;
                    let inverse = 1.0 - t;
                    let point = rough::Point {
                        x: inverse.powi(3) * start.x
                            + 3.0 * inverse * inverse * t * control_1.x
                            + 3.0 * inverse * t * t * control_2.x
                            + t.powi(3) * end.x,
                        y: inverse.powi(3) * start.y
                            + 3.0 * inverse * inverse * t * control_1.y
                            + 3.0 * inverse * t * t * control_2.y
                            + t.powi(3) * end.y,
                    };
                    points.push(transformed_clip_point(
                        point,
                        &clip.transform,
                        point_scale,
                        page_height,
                    ));
                }
                current = *end;
            }
        }
    }
    Ok(points)
}

fn transformed_clip_point(
    point: rough::Point,
    transform: &super::ResolvedTransform,
    point_scale: f64,
    page_height: f64,
) -> (f64, f64) {
    let sin = transform.angle_radians.sin();
    let cos = transform.angle_radians.cos();
    let delta_x = point.x - transform.center_x;
    let delta_y = point.y - transform.center_y;
    let x = transform.translation_x + transform.center_x + delta_x * cos - delta_y * sin;
    let y = transform.translation_y + transform.center_y + delta_x * sin + delta_y * cos;
    (x * point_scale, page_height - y * point_scale)
}

fn clip_convex_polygon(
    subject: &[(f64, f64)],
    clip: &[(f64, f64)],
) -> Result<Vec<(f64, f64)>, FormatEncodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(subject.len())
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_link_clip_input",
        })?;
    output.extend_from_slice(subject);
    let orientation = clip
        .iter()
        .zip(clip.iter().cycle().skip(1))
        .take(clip.len())
        .map(|(left, right)| left.0 * right.1 - right.0 * left.1)
        .sum::<f64>()
        .signum();
    for (edge_start, edge_end) in clip
        .iter()
        .copied()
        .zip(clip.iter().copied().cycle().skip(1))
        .take(clip.len())
    {
        let input = output;
        output = Vec::new();
        output
            .try_reserve(input.len().saturating_add(1))
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_clipped_link_polygon",
            })?;
        let Some(mut previous) = input.last().copied() else {
            break;
        };
        let mut previous_inside =
            polygon_half_plane(previous, edge_start, edge_end) * orientation >= 0.0;
        for current in input {
            let current_inside =
                polygon_half_plane(current, edge_start, edge_end) * orientation >= 0.0;
            if current_inside != previous_inside {
                output.push(line_intersection(previous, current, edge_start, edge_end));
            }
            if current_inside {
                output.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
    }
    Ok(output)
}

fn polygon_half_plane(point: (f64, f64), edge_start: (f64, f64), edge_end: (f64, f64)) -> f64 {
    (edge_end.0 - edge_start.0) * (point.1 - edge_start.1)
        - (edge_end.1 - edge_start.1) * (point.0 - edge_start.0)
}

fn line_intersection(
    start: (f64, f64),
    end: (f64, f64),
    edge_start: (f64, f64),
    edge_end: (f64, f64),
) -> (f64, f64) {
    let start_distance = polygon_half_plane(start, edge_start, edge_end);
    let end_distance = polygon_half_plane(end, edge_start, edge_end);
    let denominator = start_distance - end_distance;
    let ratio = if denominator == 0.0 {
        0.0
    } else {
        start_distance / denominator
    };
    (
        start.0 + (end.0 - start.0) * ratio,
        start.1 + (end.1 - start.1) * ratio,
    )
}

fn srgb_icc_profile() -> Result<Vec<u8>, FormatEncodeError> {
    const PROFILE_SIZE: usize = 820;
    const DATA_OFFSET: u32 = 216;
    const XYZ_SIZE: u32 = 20;
    const CURVE_OFFSET: u32 = DATA_OFFSET + 4 * XYZ_SIZE;
    const CURVE_SIZE: u32 = 12 + 2 * SRGB_TRC.len() as u32;
    const TAGS: [(&[u8; 4], u32, u32); 7] = [
        (b"rXYZ", DATA_OFFSET, XYZ_SIZE),
        (b"gXYZ", DATA_OFFSET + XYZ_SIZE, XYZ_SIZE),
        (b"bXYZ", DATA_OFFSET + 2 * XYZ_SIZE, XYZ_SIZE),
        (b"wtpt", DATA_OFFSET + 3 * XYZ_SIZE, XYZ_SIZE),
        (b"rTRC", CURVE_OFFSET, CURVE_SIZE),
        (b"gTRC", CURVE_OFFSET, CURVE_SIZE),
        (b"bTRC", CURVE_OFFSET, CURVE_SIZE),
    ];

    let mut profile = Vec::new();
    profile
        .try_reserve_exact(PROFILE_SIZE)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_srgb_profile",
        })?;
    profile.resize(128, 0);
    profile[0..4].copy_from_slice(&(PROFILE_SIZE as u32).to_be_bytes());
    profile[4..8].copy_from_slice(b"ERND");
    profile[8..12].copy_from_slice(b"\x02\x10\0\0");
    profile[12..16].copy_from_slice(b"mntr");
    profile[16..20].copy_from_slice(b"RGB ");
    profile[20..24].copy_from_slice(b"XYZ ");
    profile[24..36].copy_from_slice(&[0x07, 0xd0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0]);
    profile[36..40].copy_from_slice(b"acsp");
    profile[48..52].copy_from_slice(b"ERND");
    profile[52..56].copy_from_slice(b"sRGB");
    profile[68..80].copy_from_slice(&[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]);
    profile[80..84].copy_from_slice(b"ERND");
    profile.extend_from_slice(&(TAGS.len() as u32).to_be_bytes());
    for (signature, offset, size) in TAGS {
        profile.extend_from_slice(signature);
        profile.extend_from_slice(&offset.to_be_bytes());
        profile.extend_from_slice(&size.to_be_bytes());
    }
    append_xyz(&mut profile, [0x0000_6fa3, 0x0000_38f6, 0x0000_0391]);
    append_xyz(&mut profile, [0x0000_6294, 0x0000_b785, 0x0000_18dc]);
    append_xyz(&mut profile, [0x0000_24a1, 0x0000_0f85, 0x0000_b6d4]);
    append_xyz(&mut profile, [0x0000_f6d6, 0x0001_0000, 0x0000_d32d]);
    profile.extend_from_slice(b"curv\0\0\0\0");
    profile.extend_from_slice(&(SRGB_TRC.len() as u32).to_be_bytes());
    for sample in SRGB_TRC {
        profile.extend_from_slice(&sample.to_be_bytes());
    }
    if profile.len() != PROFILE_SIZE {
        return Err(FormatEncodeError::Encoding(
            "project sRGB profile has an invalid size".to_owned(),
        ));
    }
    Ok(profile)
}

fn append_xyz(profile: &mut Vec<u8>, coordinates: [i32; 3]) {
    profile.extend_from_slice(b"XYZ \0\0\0\0");
    for coordinate in coordinates {
        profile.extend_from_slice(&coordinate.to_be_bytes());
    }
}

#[allow(clippy::too_many_arguments)]
fn push_pdf_drawing(
    output: &mut String,
    drawing: &Drawing,
    stroke_color: CanonicalColor,
    fill_color: Option<CanonicalColor>,
    stroke_width: f64,
    stroke_style: StrokeStyle,
    source_stroke_width: f64,
    opacity: f64,
    opacity_resources: &[f64],
) -> Result<(), FormatEncodeError> {
    output.push_str("q\n");
    if let (Some(fill), Some(fill_color)) = (&drawing.fill, fill_color) {
        output.push_str("q\n");
        let fill_opacity = opacity * fill_color.alpha();
        if fill_opacity != 1.0 {
            push_opacity_resource(output, opacity_resources, fill_opacity)?;
        }
        match fill.kind {
            FillKind::Solid => {
                output.push_str("/CS1 cs\n");
                push_color(output, fill_color.rgb_f64(), "scn")?;
                push_rough_path(output, &fill.path)?;
                output.push_str(if drawing.shape.canvas_uses_even_odd_fill() {
                    "f*\n"
                } else {
                    "f\n"
                });
            }
            FillKind::Sketch => {
                push_pdf_number(output, source_stroke_width / 2.0, "PDF fill weight")?;
                output.push_str(" w\n1 J\n/CS1 CS\n");
                push_color(output, fill_color.rgb_f64(), "SCN")?;
                push_rough_path(output, &fill.path)?;
                output.push_str("S\n");
            }
        }
        output.push_str("Q\n");
    }
    let stroke_opacity = opacity * stroke_color.alpha();
    if stroke_opacity != 1.0 {
        push_opacity_resource(output, opacity_resources, stroke_opacity)?;
    }
    push_pdf_number(output, stroke_width, "PDF stroke width")?;
    output.push_str(" w\n1 J\n/CS1 CS\n");
    match stroke_style {
        StrokeStyle::Solid => output.push_str("[] 0 d\n"),
        StrokeStyle::Dashed => {
            output.push_str("[8 ");
            push_pdf_number(output, 8.0 + source_stroke_width, "PDF dash gap")?;
            output.push_str("] 0 d\n");
        }
        StrokeStyle::Dotted => {
            output.push_str("[1.5 ");
            push_pdf_number(output, 6.0 + source_stroke_width, "PDF dot gap")?;
            output.push_str("] 0 d\n");
        }
    }
    push_color(output, stroke_color.rgb_f64(), "SCN")?;
    push_rough_path(output, &drawing.outline)?;
    output.push_str("S\nQ\n");
    Ok(())
}

fn push_rough_path(output: &mut String, path: &Path) -> Result<(), FormatEncodeError> {
    for operation in &path.operations {
        match operation {
            PathOperation::Move(point) => {
                rough::push_number(output, point.x)?;
                output.push(' ');
                rough::push_number(output, point.y)?;
                output.push_str(" m\n");
            }
            PathOperation::Line(point) => {
                rough::push_number(output, point.x)?;
                output.push(' ');
                rough::push_number(output, point.y)?;
                output.push_str(" l\n");
            }
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                for (index, value) in [
                    control_1.x,
                    control_1.y,
                    control_2.x,
                    control_2.y,
                    end.x,
                    end.y,
                ]
                .into_iter()
                .enumerate()
                {
                    if index != 0 {
                        output.push(' ');
                    }
                    rough::push_number(output, value)?;
                }
                output.push_str(" c\n");
            }
        }
    }
    Ok(())
}

fn push_freedraw_path(
    output: &mut String,
    path: &super::freedraw::ProjectedPath,
) -> Result<(), FormatEncodeError> {
    let mut current = None;
    for operation in &path.operations {
        match *operation {
            super::freedraw::PathOperation::Move(point) => {
                push_pdf_number(output, point.x, "PDF freedraw x")?;
                output.push(' ');
                push_pdf_number(output, point.y, "PDF freedraw y")?;
                output.push_str(" m\n");
                current = Some(point);
            }
            super::freedraw::PathOperation::Quadratic { control, end } => {
                let start = current.ok_or_else(|| {
                    FormatEncodeError::Encoding(
                        "PDF freedraw quadratic has no current point".to_owned(),
                    )
                })?;
                let control_1 = super::rough::Point {
                    x: start.x + (control.x - start.x) * (2.0 / 3.0),
                    y: start.y + (control.y - start.y) * (2.0 / 3.0),
                };
                let control_2 = super::rough::Point {
                    x: end.x + (control.x - end.x) * (2.0 / 3.0),
                    y: end.y + (control.y - end.y) * (2.0 / 3.0),
                };
                for (index, value) in [
                    control_1.x,
                    control_1.y,
                    control_2.x,
                    control_2.y,
                    end.x,
                    end.y,
                ]
                .into_iter()
                .enumerate()
                {
                    if index != 0 {
                        output.push(' ');
                    }
                    push_pdf_number(output, value, "PDF freedraw coordinate")?;
                }
                output.push_str(" c\n");
                current = Some(end);
            }
            super::freedraw::PathOperation::Line(point) => {
                push_pdf_number(output, point.x, "PDF freedraw x")?;
                output.push(' ');
                push_pdf_number(output, point.y, "PDF freedraw y")?;
                output.push_str(" l\n");
                current = Some(point);
            }
            super::freedraw::PathOperation::Close => output.push_str("h\n"),
        }
    }
    Ok(())
}

fn push_matrix(
    output: &mut String,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
) -> Result<(), FormatEncodeError> {
    for (index, value) in [a, b, c, d, e, f].into_iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        push_pdf_number(output, value, "PDF transformation component")?;
    }
    output.push_str(" cm\n");
    Ok(())
}

fn push_pdf_rectangle(
    output: &mut String,
    width: f64,
    height: f64,
) -> Result<(), FormatEncodeError> {
    output.push_str("0 0 ");
    push_pdf_number(output, width, "PDF fallback rectangle width")?;
    output.push(' ');
    push_pdf_number(output, height, "PDF fallback rectangle height")?;
    output.push_str(" re\n");
    Ok(())
}

fn push_fallback_body(
    output: &mut String,
    width: f64,
    height: f64,
    label: Option<&super::text::ResolvedLabel>,
    colors: super::FallbackColors,
    mappings: &[PdfGlyphMapping],
) -> Result<(), FormatEncodeError> {
    push_pdf_rectangle(output, width, height)?;
    output.push_str("W\nn\n/CS1 cs\n");
    push_color(output, colors.fill.rgb_f64(), "scn")?;
    push_pdf_rectangle(output, width, height)?;
    output.push_str("f\n");
    if width >= 2.0 && height >= 2.0 {
        output.push_str("/CS1 CS\n");
        push_color(output, colors.stroke.rgb_f64(), "SCN")?;
        output.push_str("2 w\n0 J\n0 j\n[6 4] 0 d\n");
        push_pdf_rectangle(output, width, height)?;
        output.push_str("S\n[] 0 d\n0 0 m\n");
        push_pdf_number(output, width, "PDF fallback cross width")?;
        output.push(' ');
        push_pdf_number(output, height, "PDF fallback cross height")?;
        output.push_str(" l\n");
        push_pdf_number(output, width, "PDF fallback reverse cross width")?;
        output.push_str(" 0 m\n0 ");
        push_pdf_number(output, height, "PDF fallback reverse cross height")?;
        output.push_str(" l\nS\n");
    }
    if let Some(label) = label {
        output.push_str("/Span << /ActualText <FEFF");
        push_utf16be_hex(output, &label.text)?;
        output.push_str("> >> BDC\n");
        push_pdf_label_text(output, label, colors.label, mappings)?;
        output.push_str("EMC\n");
    }
    Ok(())
}

fn push_pdf_label_text(
    output: &mut String,
    label: &super::text::ResolvedLabel,
    color: CanonicalColor,
    mappings: &[PdfGlyphMapping],
) -> Result<(), FormatEncodeError> {
    output.push_str("/CS1 cs\n");
    push_color(output, color.rgb_f64(), "scn")?;
    output.push_str("BT\n/F1 14 Tf\n");
    for glyph in &label.glyphs {
        output.push_str("1 0 0 -1 ");
        push_pdf_number(output, glyph.x, "PDF fallback glyph x")?;
        output.push(' ');
        push_pdf_number(output, glyph.y, "PDF fallback glyph y")?;
        output.push_str(" Tm\n<");
        let code = glyph_mapping_code(mappings, glyph)?;
        write!(output, "{code:04X}").map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF fallback glyph ID".to_owned())
        })?;
        output.push_str("> Tj\n");
    }
    output.push_str("ET\n");
    Ok(())
}

fn glyph_mapping_code(
    mappings: &[PdfGlyphMapping],
    glyph: &super::text::ResolvedGlyph,
) -> Result<u16, FormatEncodeError> {
    mappings
        .iter()
        .find(|mapping| mapping.glyph_id == glyph.id && mapping.unicode == glyph.unicode)
        .map(|mapping| mapping.code)
        .ok_or_else(|| {
            FormatEncodeError::Encoding(
                "PDF shaped glyph has no deterministic character-code mapping".to_owned(),
            )
        })
}

fn push_transformed_clip(
    output: &mut String,
    clip: &super::ResolvedClip,
) -> Result<(), FormatEncodeError> {
    let sin = clip.transform.angle_radians.sin();
    let cos = clip.transform.angle_radians.cos();
    let offset_x = clip.transform.translation_x + clip.transform.center_x
        - cos * clip.transform.center_x
        + sin * clip.transform.center_y;
    let offset_y = clip.transform.translation_y + clip.transform.center_y
        - sin * clip.transform.center_x
        - cos * clip.transform.center_y;
    let map = |x: f64, y: f64| (cos * x - sin * y + offset_x, sin * x + cos * y + offset_y);
    for operation in &clip.path.operations {
        match operation {
            PathOperation::Move(point) => {
                let (x, y) = map(point.x, point.y);
                push_pdf_point(output, x, y, "m")?;
            }
            PathOperation::Line(point) => {
                let (x, y) = map(point.x, point.y);
                push_pdf_point(output, x, y, "l")?;
            }
            PathOperation::Cubic {
                control_1,
                control_2,
                end,
            } => {
                for point in [control_1, control_2, end] {
                    let (x, y) = map(point.x, point.y);
                    push_pdf_number(output, x, "PDF frame clip curve x")?;
                    output.push(' ');
                    push_pdf_number(output, y, "PDF frame clip curve y")?;
                    output.push(' ');
                }
                output.push_str("c\n");
            }
        }
    }
    output.push_str("W\nn\n");
    Ok(())
}

fn xmp_metadata(manifest: &str) -> Result<String, FormatEncodeError> {
    const PREFIX: &str = concat!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"",
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\" ",
        "xmlns:er=\"urn:excalidraw-render:pdf:metadata:1\"><er:manifest>"
    );
    const SUFFIX: &str = "</er:manifest></rdf:Description></rdf:RDF></x:xmpmeta>";
    let capacity = PREFIX
        .len()
        .checked_add(manifest.len().saturating_mul(6))
        .and_then(|capacity| capacity.checked_add(SUFFIX.len()))
        .ok_or_else(pdf_size_error)?;
    let mut xmp = String::new();
    xmp.try_reserve_exact(capacity)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "metadata",
            subject: "pdf_mandatory_notice_xmp",
        })?;
    xmp.push_str(PREFIX);
    for character in manifest.chars() {
        match character {
            '&' => xmp.push_str("&amp;"),
            '<' => xmp.push_str("&lt;"),
            '>' => xmp.push_str("&gt;"),
            character => xmp.push(character),
        }
    }
    xmp.push_str(SUFFIX);
    Ok(xmp)
}

fn push_utf16be_hex(output: &mut String, text: &str) -> Result<(), FormatEncodeError> {
    for unit in text.encode_utf16() {
        write!(output, "{unit:04X}").map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF fallback actual text".to_owned())
        })?;
    }
    Ok(())
}

fn to_unicode_cmap(
    mappings: &[PdfGlyphMapping],
    base_font: &str,
) -> Result<String, FormatEncodeError> {
    let mut cmap = String::new();
    let capacity = mappings
        .len()
        .checked_mul(64)
        .and_then(|bytes| bytes.checked_add(512))
        .ok_or_else(pdf_size_error)?;
    cmap.try_reserve_exact(capacity)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_fallback_to_unicode_cmap",
        })?;
    cmap.push_str(concat!(
        "/CIDInit /ProcSet findresource begin\n",
        "12 dict begin\nbegincmap\n",
        "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n",
        "/CMapName /"
    ));
    cmap.push_str(base_font);
    cmap.push_str(
        "-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in mappings.chunks(100) {
        writeln!(cmap, "{} beginbfchar", chunk.len()).map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF ToUnicode count".to_owned())
        })?;
        for mapping in chunk {
            write!(cmap, "<{:04X}> <", mapping.code).map_err(|_| {
                FormatEncodeError::Encoding("failed to write PDF ToUnicode source".to_owned())
            })?;
            if mapping.unicode.is_empty() {
                cmap.push_str("FFFD");
            } else {
                for unit in &mapping.unicode {
                    write!(cmap, "{unit:04X}").map_err(|_| {
                        FormatEncodeError::Encoding(
                            "failed to write PDF ToUnicode destination".to_owned(),
                        )
                    })?;
                }
            }
            cmap.push_str(">\n");
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend");
    Ok(cmap)
}

fn push_pdf_point(
    output: &mut String,
    x: f64,
    y: f64,
    operator: &str,
) -> Result<(), FormatEncodeError> {
    push_pdf_number(output, x, "PDF fallback label x")?;
    output.push(' ');
    push_pdf_number(output, y, "PDF fallback label y")?;
    output.push(' ');
    output.push_str(operator);
    output.push('\n');
    Ok(())
}

fn push_pdf_vec_number(
    output: &mut Vec<u8>,
    value: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    let number = format_pdf_number(value, subject)?;
    output.extend_from_slice(number.as_bytes());
    Ok(())
}

fn push_pdf_literal_string(output: &mut Vec<u8>, value: &str) {
    for byte in value.bytes() {
        match byte {
            b'(' => output.extend_from_slice(b"\\("),
            b')' => output.extend_from_slice(b"\\)"),
            b'\\' => output.extend_from_slice(b"\\\\"),
            byte => output.push(byte),
        }
    }
}

fn push_pdf_link(output: &mut Vec<u8>, link: &str) {
    let path_end = link.find(['/', '?', '#']).unwrap_or(link.len());
    let has_scheme = link
        .find(':')
        .is_some_and(|scheme_end| scheme_end < path_end);
    if !has_scheme {
        if link.starts_with('/') {
            output.extend_from_slice(b"http://reference.invalid");
        } else {
            output.extend_from_slice(b"http://reference.invalid/");
        }
    }
    push_pdf_literal_string(output, link);
}

fn push_opacity_resource(
    output: &mut String,
    resources: &[f64],
    opacity: f64,
) -> Result<(), FormatEncodeError> {
    let resource = resources
        .iter()
        .position(|candidate| candidate.to_bits() == opacity.to_bits())
        .ok_or_else(|| {
            FormatEncodeError::Encoding("PDF opacity resource has no owner".to_owned())
        })?;
    writeln!(output, "/GS{} gs", resource + 1)
        .map_err(|_| FormatEncodeError::Encoding("failed to write PDF opacity resource".to_owned()))
}

fn push_color(
    output: &mut String,
    color: [f64; 3],
    operator: &str,
) -> Result<(), FormatEncodeError> {
    for (index, value) in color.into_iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        push_pdf_number(output, value, "PDF sRGB color component")?;
    }
    writeln!(output, " {operator}")
        .map_err(|_| FormatEncodeError::Encoding("failed to write PDF color".to_owned()))
}

fn push_pdf_number(
    output: &mut String,
    value: f64,
    subject: &str,
) -> Result<(), FormatEncodeError> {
    output.push_str(&format_pdf_number(value, subject)?);
    Ok(())
}

fn format_pdf_number(value: f64, subject: &str) -> Result<String, FormatEncodeError> {
    if !value.is_finite() {
        return Err(FormatEncodeError::Encoding(format!(
            "{subject} must be finite"
        )));
    }
    let value = if value == 0.0 { 0.0 } else { value };
    let mut rendered = String::new();
    rendered
        .try_reserve_exact(32)
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_number",
        })?;
    write!(rendered, "{value}")
        .map_err(|_| FormatEncodeError::Encoding(format!("failed to write {subject}")))?;
    let Some(exponent_marker) = rendered.find(['e', 'E']) else {
        return Ok(rendered);
    };
    expand_exponent(&rendered, exponent_marker, subject)
}

fn expand_exponent(
    rendered: &str,
    exponent_marker: usize,
    subject: &str,
) -> Result<String, FormatEncodeError> {
    let (mantissa, exponent) = rendered.split_at(exponent_marker);
    let exponent = exponent[1..]
        .parse::<i32>()
        .map_err(|_| FormatEncodeError::Encoding(format!("failed to expand {subject} exponent")))?;
    let (negative, unsigned) = mantissa
        .strip_prefix('-')
        .map_or((false, mantissa), |unsigned| (true, unsigned));
    let decimal = unsigned.find('.').unwrap_or(unsigned.len());
    let mut digits = String::new();
    digits
        .try_reserve_exact(unsigned.len())
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_number",
        })?;
    digits.extend(unsigned.chars().filter(|character| *character != '.'));
    let decimal = i32::try_from(decimal)
        .map_err(|_| FormatEncodeError::Encoding(format!("{subject} is too long")))?
        + exponent;
    let mut output = String::new();
    output
        .try_reserve(
            rendered
                .len()
                .saturating_add(exponent.unsigned_abs() as usize),
        )
        .map_err(|_| FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_number",
        })?;
    if negative {
        output.push('-');
    }
    if decimal <= 0 {
        output.push_str("0.");
        for _ in 0..decimal.unsigned_abs() {
            output.push('0');
        }
        output.push_str(&digits);
    } else if usize::try_from(decimal).is_ok_and(|decimal| decimal >= digits.len()) {
        output.push_str(&digits);
        for _ in digits.len()..usize::try_from(decimal).unwrap_or(digits.len()) {
            output.push('0');
        }
    } else {
        let decimal = usize::try_from(decimal)
            .map_err(|_| FormatEncodeError::Encoding(format!("{subject} is too long")))?;
        output.push_str(&digits[..decimal]);
        output.push('.');
        output.push_str(&digits[decimal..]);
    }
    Ok(output)
}

fn begin_object(
    output: &mut Vec<u8>,
    offsets: &mut [usize],
    id: usize,
) -> Result<(), FormatEncodeError> {
    offsets[id - 1] = output.len();
    writeln!(output, "{id} 0 obj").map_err(write_error)
}

fn pdf_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("PDF plan exceeds serializer capacity".to_owned())
}

fn width_scale(element: &PdfElement) -> f64 {
    let PdfElement::Image {
        asset,
        width,
        height,
        source_rect,
        ..
    } = element
    else {
        return 1.0;
    };
    let source_width = source_rect
        .width
        .min(f64::from(asset.width))
        .max(f64::EPSILON);
    let source_height = source_rect
        .height
        .min(f64::from(asset.height))
        .max(f64::EPSILON);
    (*width / source_width).max(*height / source_height)
}

fn collect_raster_closures(
    elements: &[PdfElement],
    final_scale: f64,
    logical_width: f64,
    logical_height: f64,
) -> Result<Vec<PdfRasterClosure>, FormatEncodeError> {
    let mut closures: Vec<PdfRasterClosure> = Vec::new();
    for (element_index, element) in elements.iter().enumerate() {
        let PdfElement::Image {
            source,
            asset,
            transform,
            frame_clips,
            width,
            height,
            source_rect,
            scale,
            ..
        } = element
        else {
            continue;
        };
        if asset.svg_effects.is_empty() {
            continue;
        }
        let tree = asset.sanitized_svg_tree.as_deref().ok_or_else(|| {
            FormatEncodeError::Encoding("PDF effect source lost its sanitized tree".to_owned())
        })?;
        let mut occurrences = Vec::new();
        collect_filter_occurrences(tree.root(), None, &mut occurrences);
        let mut occurrence_closures: Vec<Option<usize>> = Vec::new();
        occurrence_closures.resize(occurrences.len(), None);
        let mut matched_occurrences = std::collections::BTreeMap::<String, usize>::new();
        for (occurrence_index, occurrence) in occurrences.iter().enumerate() {
            let exact_usage = asset.svg_effects.iter().find(|effect| {
                !occurrence.usage_id.is_empty() && effect.usage_id == occurrence.usage_id
            });
            let candidates = asset
                .svg_effects
                .iter()
                .filter(|effect| {
                    effect.marker == occurrence.marker || effect.filter_id == occurrence.filter_id
                })
                .collect::<Vec<_>>();
            let key = if occurrence.marker.is_empty() {
                occurrence.filter_id.clone()
            } else {
                occurrence.marker.clone()
            };
            let matched = matched_occurrences.entry(key).or_default();
            let effect = exact_usage.or_else(|| {
                (!candidates.is_empty()).then(|| candidates[*matched % candidates.len()])
            });
            *matched = matched.saturating_add(1);
            let Some(effect) = effect else {
                continue;
            };
            if let Some(parent_closure) = occurrence
                .parent
                .and_then(|parent| occurrence_closures.get(parent).copied().flatten())
            {
                let closure = closures
                    .get_mut(parent_closure)
                    .ok_or_else(pdf_size_error)?;
                closure
                    .causes
                    .extend(effect.causes.iter().map(effect_cause_path));
                closure.unisolatable |= effect.unisolatable;
                occurrence_closures[occurrence_index] = Some(parent_closure);
                continue;
            }
            let (x, y, right, bottom) = occurrence.bounds;
            let viewport_right = f64::from(asset.width);
            let viewport_bottom = f64::from(asset.height);
            let x = x.max(0.0).max(source_rect.x);
            let y = y.max(0.0).max(source_rect.y);
            let right = right
                .min(viewport_right)
                .min(source_rect.x + source_rect.width);
            let bottom = bottom
                .min(viewport_bottom)
                .min(source_rect.y + source_rect.height);
            if right <= x || bottom <= y {
                continue;
            }
            let local =
                map_asset_bounds((x, y, right, bottom), *width, *height, *source_rect, *scale);
            let (x, y, right, bottom) = transform_bounds(local, *transform);
            let mut x = x.max(0.0);
            let mut y = y.max(0.0);
            let mut right = right.min(logical_width);
            let mut bottom = bottom.min(logical_height);
            for clip in frame_clips {
                let Some(clip_bounds) = clip.path.cubic_bounds() else {
                    right = x;
                    bottom = y;
                    break;
                };
                let clip_bounds = transform_bounds(clip_bounds, clip.transform);
                x = x.max(clip_bounds.0);
                y = y.max(clip_bounds.1);
                right = right.min(clip_bounds.2);
                bottom = bottom.min(clip_bounds.3);
            }
            if right <= x || bottom <= y {
                continue;
            }
            let pixel_x = (x * final_scale).floor();
            let pixel_y = (y * final_scale).floor();
            let pixel_right = (right * final_scale).ceil();
            let pixel_bottom = (bottom * final_scale).ceil();
            let pixel_width = (pixel_right - pixel_x).max(1.0) as u64;
            let pixel_height = (pixel_bottom - pixel_y).max(1.0) as u64;
            let logical_bounds = LogicalBounds {
                x: FiniteValue::new(pixel_x / final_scale).ok_or_else(pdf_size_error)?,
                y: FiniteValue::new(pixel_y / final_scale).ok_or_else(pdf_size_error)?,
                width: FiniteValue::new(pixel_width as f64 / final_scale)
                    .ok_or_else(pdf_size_error)?,
                height: FiniteValue::new(pixel_height as f64 / final_scale)
                    .ok_or_else(pdf_size_error)?,
            };
            let causes = effect
                .causes
                .iter()
                .map(effect_cause_path)
                .collect::<Vec<_>>();
            closures.push(PdfRasterClosure {
                element_id: source.id().to_owned(),
                json_pointer: asset.svg_fallback_pointer.clone().unwrap_or_default(),
                source_order: u64::try_from(element_index).unwrap_or(u64::MAX),
                source_position: effect.node_ordinal,
                logical_bounds,
                pixel_width,
                pixel_height,
                causes,
                unisolatable: effect.unisolatable,
            });
            occurrence_closures[occurrence_index] = Some(closures.len() - 1);
        }
    }
    Ok(closures)
}

fn collect_filter_occurrences(
    group: &usvg::Group,
    parent: Option<usize>,
    occurrences: &mut Vec<FilterOccurrence>,
) {
    let mut child_parent = parent;
    if !group.filters().is_empty()
        && group.has_children()
        && group.bounding_box().width() > 0.0
        && group.bounding_box().height() > 0.0
    {
        let rect = group.abs_layer_bounding_box();
        let mut bounds = (
            f64::from(rect.x()),
            f64::from(rect.y()),
            f64::from(rect.right()),
            f64::from(rect.bottom()),
        );
        if let Some(clip) = group.clip_path() {
            let Some(clip_bounds) = nested_clip_bounds(clip) else {
                return;
            };
            bounds.0 = bounds.0.max(clip_bounds.0);
            bounds.1 = bounds.1.max(clip_bounds.1);
            bounds.2 = bounds.2.min(clip_bounds.2);
            bounds.3 = bounds.3.min(clip_bounds.3);
            if bounds.2 <= bounds.0 || bounds.3 <= bounds.1 {
                return;
            }
        }
        for filter in group.filters() {
            occurrences.push(FilterOccurrence {
                filter_id: filter.id().to_owned(),
                usage_id: group.id().to_owned(),
                marker: filter
                    .primitives()
                    .first()
                    .map(|primitive| primitive.result().to_owned())
                    .unwrap_or_default(),
                bounds,
                parent,
            });
            child_parent = Some(occurrences.len() - 1);
        }
    }
    for child in group.children() {
        if let usvg::Node::Group(child) = child {
            collect_filter_occurrences(child, child_parent, occurrences);
        }
        child.subroots(|subroot| {
            collect_filter_occurrences(subroot, child_parent, occurrences);
        });
    }
}

fn nested_clip_bounds(clip: &usvg::ClipPath) -> Option<(f64, f64, f64, f64)> {
    if clip.root().bounding_box().width() == 0.0 || clip.root().bounding_box().height() == 0.0 {
        return None;
    }
    let rect = clip
        .root()
        .abs_layer_bounding_box()
        .transform(clip.transform())?;
    let mut bounds = (
        f64::from(rect.x()),
        f64::from(rect.y()),
        f64::from(rect.right()),
        f64::from(rect.bottom()),
    );
    if let Some(parent) = clip.clip_path() {
        let parent = nested_clip_bounds(parent)?;
        bounds.0 = bounds.0.max(parent.0);
        bounds.1 = bounds.1.max(parent.1);
        bounds.2 = bounds.2.min(parent.2);
        bounds.3 = bounds.3.min(parent.3);
        if bounds.2 <= bounds.0 || bounds.3 <= bounds.1 {
            return None;
        }
    }
    Some(bounds)
}

fn map_asset_bounds(
    bounds: (f64, f64, f64, f64),
    width: f64,
    height: f64,
    source: super::ResolvedImageSourceRect,
    scale: [i8; 2],
) -> (f64, f64, f64, f64) {
    let map_x = |value: f64| {
        let value = (value - source.x) * width / source.width;
        if scale[0] < 0 {
            width - value
        } else {
            value
        }
    };
    let map_y = |value: f64| {
        let value = (value - source.y) * height / source.height;
        if scale[1] < 0 {
            height - value
        } else {
            value
        }
    };
    let (x1, x2) = (map_x(bounds.0), map_x(bounds.2));
    let (y1, y2) = (map_y(bounds.1), map_y(bounds.3));
    (x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2))
}

fn transform_bounds(
    bounds: (f64, f64, f64, f64),
    transform: super::ResolvedTransform,
) -> (f64, f64, f64, f64) {
    let (sin, cos) = transform.angle_radians.sin_cos();
    let offset_x = transform.translation_x + transform.center_x - cos * transform.center_x
        + sin * transform.center_y;
    let offset_y = transform.translation_y + transform.center_y
        - sin * transform.center_x
        - cos * transform.center_y;
    let point = |x: f64, y: f64| (cos * x - sin * y + offset_x, sin * x + cos * y + offset_y);
    let points = [
        point(bounds.0, bounds.1),
        point(bounds.2, bounds.1),
        point(bounds.2, bounds.3),
        point(bounds.0, bounds.3),
    ];
    points.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, min_y, max_x, max_y), (x, y)| {
            (min_x.min(*x), min_y.min(*y), max_x.max(*x), max_y.max(*y))
        },
    )
}

fn effect_cause_path(cause: &super::asset::svg::EffectCause) -> NestedPath {
    let mut segments = Vec::new();
    for (node, attribute) in &cause.slots {
        segments.push(NestedPathSegment {
            kind: NestedSegmentKind::Node,
            ordinal: *node,
            name: None,
        });
        segments.push(NestedPathSegment {
            kind: NestedSegmentKind::Attribute,
            ordinal: *attribute,
            name: Some("href".to_owned()),
        });
    }
    segments.push(NestedPathSegment {
        kind: NestedSegmentKind::Node,
        ordinal: cause.node_ordinal,
        name: None,
    });
    if let Some((ordinal, name)) = &cause.attribute {
        segments.push(NestedPathSegment {
            kind: if cause.effect_input {
                NestedSegmentKind::EffectInput
            } else {
                NestedSegmentKind::Attribute
            },
            ordinal: *ordinal,
            name: Some(name.clone()),
        });
    }
    NestedPath { segments }
}

fn svg_chunks(resources: &[PdfSvgResource]) -> Result<Vec<PdfSvgChunk>, FormatEncodeError> {
    let mut chunks = Vec::new();
    chunks.try_reserve_exact(resources.len()).map_err(|_| {
        FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_resource",
            subject: "pdf_svg_chunks",
        }
    })?;
    for resource in resources {
        let tree = resource
            .asset
            .sanitized_svg_tree
            .as_deref()
            .ok_or_else(|| {
                FormatEncodeError::Encoding("PDF SVG resource lost its sanitized tree".to_owned())
            })?;
        let (chunk, root) = svg2pdf::to_chunk(
            tree,
            svg2pdf::ConversionOptions {
                compress: false,
                raster_scale: resource.raster_scale as f32,
                embed_text: false,
                ..svg2pdf::ConversionOptions::default()
            },
        )
        .map_err(|error| {
            FormatEncodeError::Encoding(format!("failed to lower sanitized SVG to PDF: {error}"))
        })?;
        chunks.push(PdfSvgChunk {
            chunk,
            root,
            bytes: Vec::new(),
            object_offsets: Vec::new(),
        });
    }
    Ok(chunks)
}

fn renumber_svg_chunks(
    chunks: Vec<PdfSvgChunk>,
    first_object: usize,
) -> Result<Vec<PdfSvgChunk>, FormatEncodeError> {
    let total_objects = chunks
        .iter()
        .try_fold(0_usize, |count, chunk| {
            count.checked_add(chunk.chunk.refs().len())
        })
        .ok_or_else(pdf_size_error)?;
    let last_object = first_object
        .checked_add(total_objects)
        .ok_or_else(pdf_size_error)?;
    if last_object > i32::MAX as usize {
        return Err(pdf_size_error());
    }
    let mut next = i32::try_from(first_object).map_err(|_| pdf_size_error())?;
    let mut renumbered = Vec::new();
    renumbered.try_reserve_exact(chunks.len()).map_err(|_| {
        FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_resource",
            subject: "pdf_svg_chunks",
        }
    })?;
    for resource in chunks {
        let mut mapping = std::collections::BTreeMap::new();
        let chunk = resource.chunk.renumber(|old| {
            *mapping.entry(old).or_insert_with(|| {
                let allocated = pdf_writer::Ref::new(next);
                next += 1;
                allocated
            })
        });
        let root = *mapping.get(&resource.root).ok_or_else(pdf_size_error)?;
        let (bytes, object_offsets) = canonicalize_pdf_chunk(&chunk)?;
        renumbered.push(PdfSvgChunk {
            chunk,
            root,
            bytes,
            object_offsets,
        });
    }
    Ok(renumbered)
}

fn append_chunk(
    pdf: &mut Vec<u8>,
    offsets: &mut [usize],
    resource: &PdfSvgChunk,
) -> Result<(), FormatEncodeError> {
    for &(object, position) in &resource.object_offsets {
        let index = object.checked_sub(1).ok_or_else(pdf_size_error)?;
        *offsets.get_mut(index).ok_or_else(pdf_size_error)? =
            pdf.len().checked_add(position).ok_or_else(pdf_size_error)?;
    }
    pdf.extend_from_slice(&resource.bytes);
    Ok(())
}

#[allow(clippy::type_complexity)]
fn canonicalize_pdf_chunk(
    chunk: &pdf_writer::Chunk,
) -> Result<(Vec<u8>, Vec<(usize, usize)>), FormatEncodeError> {
    let bytes = chunk.as_bytes();
    let mut canonical = Vec::with_capacity(bytes.len());
    let mut offsets = Vec::new();
    offsets.try_reserve_exact(chunk.refs().len()).map_err(|_| {
        FormatEncodeError::MemoryUnavailable {
            subject_kind: "format_workspace",
            subject: "pdf_svg_object_offsets",
        }
    })?;
    let mut cursor = 0_usize;
    for reference in chunk.refs() {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            canonical.push(bytes[cursor]);
            cursor += 1;
        }
        let marker = format!("{} 0 obj\n", reference.get());
        if !bytes[cursor..].starts_with(marker.as_bytes()) {
            return Err(FormatEncodeError::Encoding(
                "PDF SVG chunk object order did not match its reference table".to_owned(),
            ));
        }
        offsets.push((
            usize::try_from(reference.get()).map_err(|_| pdf_size_error())?,
            canonical.len(),
        ));
        canonical.extend_from_slice(marker.as_bytes());
        cursor += marker.len();

        let relative_end = find_bytes(&bytes[cursor..], b"endobj\n").ok_or_else(|| {
            FormatEncodeError::Encoding("PDF SVG chunk object was unterminated".to_owned())
        })?;
        let relative_stream = find_bytes(&bytes[cursor..cursor + relative_end], b"stream\n");
        if let Some(relative_stream) = relative_stream {
            let stream = cursor + relative_stream;
            canonicalize_pdf_text(&bytes[cursor..stream], &mut canonical);
            canonical.extend_from_slice(b"stream\n");
            let body = stream + b"stream\n".len();
            let length = pdf_stream_length(&bytes[cursor..stream])?;
            let body_end = body.checked_add(length).ok_or_else(pdf_size_error)?;
            let trailer = b"\nendstream\nendobj\n";
            let object_end = body_end
                .checked_add(trailer.len())
                .ok_or_else(pdf_size_error)?;
            if bytes.get(body_end..object_end) != Some(trailer.as_slice()) {
                return Err(FormatEncodeError::Encoding(
                    "PDF SVG stream length did not match its object boundary".to_owned(),
                ));
            }
            canonical.extend_from_slice(bytes.get(body..body_end).ok_or_else(pdf_size_error)?);
            canonical.extend_from_slice(trailer);
            cursor = object_end;
        } else {
            let object_end = cursor
                .checked_add(relative_end)
                .and_then(|end| end.checked_add(b"endobj\n".len()))
                .ok_or_else(pdf_size_error)?;
            canonicalize_pdf_text(&bytes[cursor..object_end], &mut canonical);
            cursor = object_end;
        }
    }
    canonicalize_pdf_text(&bytes[cursor..], &mut canonical);
    Ok((canonical, offsets))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn pdf_stream_length(dictionary: &[u8]) -> Result<usize, FormatEncodeError> {
    let marker = b"/Length ";
    let start = find_bytes(dictionary, marker)
        .and_then(|index| index.checked_add(marker.len()))
        .ok_or_else(|| {
            FormatEncodeError::Encoding("PDF SVG stream had no direct length".to_owned())
        })?;
    let digits = dictionary[start..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .copied()
        .collect::<Vec<_>>();
    if digits.is_empty() {
        return Err(FormatEncodeError::Encoding(
            "PDF SVG stream length was not an integer".to_owned(),
        ));
    }
    std::str::from_utf8(&digits)
        .ok()
        .and_then(|digits| digits.parse().ok())
        .ok_or_else(pdf_size_error)
}

fn canonicalize_pdf_text(bytes: &[u8], output: &mut Vec<u8>) {
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n').peekable();
    while let Some(line) = lines.next() {
        let indentation = line.iter().take_while(|byte| **byte == b' ').count();
        if line.get(indentation) != Some(&b'/') || line.ends_with(b"<<\n") {
            output.extend_from_slice(line);
            continue;
        }
        let mut run = vec![line];
        while let Some(next) = lines.peek().copied() {
            let next_indentation = next.iter().take_while(|byte| **byte == b' ').count();
            if next_indentation != indentation
                || next.get(next_indentation) != Some(&b'/')
                || next.ends_with(b"<<\n")
            {
                break;
            }
            if let Some(next) = lines.next() {
                run.push(next);
            }
        }
        run.sort_unstable();
        for line in run {
            output.extend_from_slice(line);
        }
    }
}

fn checked_object(first: usize, index: usize, stride: usize) -> Result<usize, FormatEncodeError> {
    index
        .checked_mul(stride)
        .and_then(|offset| first.checked_add(offset))
        .ok_or_else(pdf_size_error)
}

fn point_dimension(logical: f64) -> Result<f64, FormatEncodeError> {
    let points = logical * POINTS_PER_LOGICAL_PIXEL;
    if !points.is_finite() || points <= 0.0 {
        return Err(FormatEncodeError::Encoding(
            "PDF page dimensions must be finite positive point values".to_owned(),
        ));
    }
    Ok(points)
}

fn write_error(_: std::io::Error) -> FormatEncodeError {
    FormatEncodeError::Encoding("failed to encode PDF bytes".to_owned())
}
