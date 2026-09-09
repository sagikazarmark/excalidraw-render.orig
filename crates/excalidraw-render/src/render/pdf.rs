//! The vector-first PDF backend (stage 5 and 6): `PdfPlan` lowers the resolved
//! scene into paint-ordered `PdfElement`s and `encode` writes the object graph
//! — catalog, page, resources, ExtGStates, XMP metadata, the embedded scene,
//! link annotations, fallback forms, fonts, images and the nested-SVG chunks —
//! with a hand-maintained object layout the `checked_object` arithmetic keeps
//! honest. This file also owns the project sRGB ICC profile, the link-annotation
//! geometry and the image-viewport projection the children share.
//!
//! `content` writes the page content stream and the fallback form streams,
//! `fonts` the Type0 font objects and their deterministic glyph mappings,
//! `nested_svg` the raster-closure analysis over sanitized `usvg` trees and
//! the bounded svg2pdf adapter that renumbers and canonicalizes its chunks, and
//! `raster_preflight` the session's admission of those closures.

mod content;
mod fonts;
mod nested_svg;
mod raster_preflight;

use nested_svg::{append_chunk, collect_raster_closures, renumber_svg_chunks, svg_chunks};

use std::io::Write as _;
use std::sync::Arc;

use crate::normalization::NormalizedElementSource;
use crate::{
    color::{self, CanonicalColor},
    DiagnosticCode, LogicalBounds, MetadataComponent, NestedPath, RenderRequest,
};

use super::font_demand::ResolvedFont;
use super::geometry::{clip_convex_polygon, Path, PathOperation, Point};
use super::metadata::ArtifactMetadata;
use super::notices::MandatoryNotices;
use super::number_format::format_pdf_number;
use super::resolve::{
    decoration_fill_color, DarkModeImageFilter, FallbackColors, FallbackSource, ResolvedClip,
    ResolvedElement, ResolvedFrameName, ResolvedFrameOutline, ResolvedImageAsset,
    ResolvedImageProjection, ResolvedImageSourceRect, ResolvedLabelMask, ResolvedScene,
    ResolvedTransform, StrokeStyle,
};
use super::rough::{Drawing, Fill};
use super::text::{ResolvedLabel, ResolvedText};
use super::{freedraw, metadata_embedding_error, FormatEncodeError, SvgTreeCache};

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

// Project-owned content is lowered directly; the bounded svg2pdf adapter in
// `nested_svg` is confined to sanitized nested-SVG Form chunks.
#[derive(Debug)]
pub(super) struct PdfPlan {
    logical_width: f64,
    logical_height: f64,
    scale: f64,
    page_width: f64,
    page_height: f64,
    background: Option<CanonicalColor>,
    fallback_colors: FallbackColors,
    mandatory_notices: Option<MandatoryNotices>,
    metadata: Option<ArtifactMetadata>,
    fallback_font_sfnt: Option<Vec<u8>>,
    official_fonts: Vec<ResolvedFont>,
    svg_trees: SvgTreeCache,
    elements: Vec<PdfElement>,
    raster_closures: Vec<PdfRasterClosure>,
}

/// One raster image XObject: an asset embedded once however many elements
/// reference it, with the dark-mode filter those elements share (the filter
/// follows from the mode and the asset kind, so they agree).
#[derive(Debug)]
struct PdfImageResource {
    asset: Arc<ResolvedImageAsset>,
    dark_mode_filter: Option<DarkModeImageFilter>,
}

#[derive(Debug)]
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

#[derive(Debug)]
struct PdfSvgResource {
    asset: Arc<ResolvedImageAsset>,
    raster_scale: f64,
}

#[derive(Debug)]
enum PdfElement {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        frame_clips: Vec<ResolvedClip>,
        transform: ResolvedTransform,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
        background_color: CanonicalColor,
        label_mask: Option<ResolvedLabelMask>,
        link: Option<String>,
        width: f64,
        height: f64,
        generated_label: Option<ResolvedText>,
        generated_label_color: Option<CanonicalColor>,
        generated_label_offset_y: f64,
    },
    Freedraw {
        source: NormalizedElementSource,
        path: freedraw::ProjectedPath,
        background_fill: Option<Fill>,
        frame_clips: Vec<ResolvedClip>,
        transform: ResolvedTransform,
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
        name: Option<ResolvedFrameName>,
        outline: Option<ResolvedFrameOutline>,
        frame_clips: Vec<ResolvedClip>,
        transform: ResolvedTransform,
        opacity: f64,
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
        /// The dark-mode filter official SVG applies to this image, which PDF
        /// lowers into a raster asset's pixels.
        dark_mode_filter: Option<DarkModeImageFilter>,
        frame_clips: Vec<ResolvedClip>,
        /// The rounded-corner clip of an image with a roundness record, in
        /// element space; without one the image clips to its rectangle.
        corner_clip: Option<Path>,
        transform: ResolvedTransform,
        opacity: f64,
        link: Option<String>,
        /// The element's unrounded size: the flip axis and the link rectangle.
        width: f64,
        height: f64,
        /// The crop in the asset's pixel grid, which bounds the effect regions
        /// a nested SVG rasterizes.
        source_rect: ResolvedImageSourceRect,
        /// The reference export's rounded viewport and content placement.
        projection: ResolvedImageProjection,
        scale: [i8; 2],
    },
    Fallback {
        source: FallbackSource,
        label: Option<ResolvedLabel>,
        link: Option<String>,
        frame_clips: Vec<ResolvedClip>,
        width: f64,
        height: f64,
        transform: ResolvedTransform,
        opacity: f64,
    },
}

impl PdfPlan {
    pub(super) fn image_resource_pixel_counts(&self) -> Result<Vec<u64>, FormatEncodeError> {
        let mut counts = self
            .image_resources()?
            .iter()
            .map(|resource| {
                u64::from(resource.asset.width)
                    .checked_mul(u64::from(resource.asset.height))
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
        svg_trees: &SvgTreeCache,
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
                        path: freedraw::project(freedraw.path)?,
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
                        outline: frame.outline,
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
                        dark_mode_filter: image.dark_mode_filter,
                        frame_clips: image.frame_clips,
                        corner_clip: image.corner_clip.map(|clip| clip.path),
                        transform: image.transform,
                        opacity: image.opacity,
                        link: image.link,
                        width: image.width,
                        height: image.height,
                        source_rect: image.source_rect,
                        projection: image.projection,
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
        let raster_closures = collect_raster_closures(
            &elements,
            svg_trees,
            scale,
            scene.logical_width,
            scene.logical_height,
        )?;
        Ok(Self {
            logical_width: scene.logical_width,
            logical_height: scene.logical_height,
            scale,
            svg_trees: svg_trees.clone(),
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

    pub(super) fn take_mandatory_notices(&mut self) -> Option<MandatoryNotices> {
        self.mandatory_notices.take()
    }

    pub(super) fn set_metadata(&mut self, metadata: ArtifactMetadata) {
        self.metadata = Some(metadata);
        self.mandatory_notices = None;
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let opacity_resources = self.opacity_resources()?;
        let image_resources = self.image_resources()?;
        let svg_resources = self.svg_resources()?;
        let raw_svg_chunks = svg_chunks(&svg_resources, &self.svg_trees)?;
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
            .and_then(ArtifactMetadata::manifest)
            .map(xmp_metadata)
            .transpose()
            .map_err(|_| {
                metadata_embedding_error(
                    DiagnosticCode::METADATA_MANIFEST_EMBEDDING_FAILED,
                    MetadataComponent::Manifest,
                )
            })?;
        let embedded_scene = self
            .metadata
            .as_ref()
            .and_then(ArtifactMetadata::canonical_scene);
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
                    .try_fold(capacity, |capacity, resource| {
                        usize::try_from(resource.asset.width)
                            .ok()?
                            .checked_mul(usize::try_from(resource.asset.height).ok()?)
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
        for (index, resource) in image_resources.iter().enumerate() {
            let asset = &resource.asset;
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
            for pixel in rgba.as_chunks::<4>().0 {
                let [red, green, blue, pixel_alpha] = [pixel[0], pixel[1], pixel[2], pixel[3]];
                // Official SVG is the visual authority for PDF images (spec
                // section 8.6): in dark mode a raster pixel passes through
                // the reference's image filter and root filter. An invisible
                // pixel keeps its canonical zero RGB.
                let lowered = match resource.dark_mode_filter {
                    Some(DarkModeImageFilter::RasterImage) if pixel_alpha != 0 => {
                        color::dark_mode_raster_image_pixel([red, green, blue])
                    }
                    _ => [red, green, blue],
                };
                rgb.extend_from_slice(&lowered);
                alpha.push(pixel_alpha);
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
        transform: &ResolvedTransform,
        frame_clips: &[ResolvedClip],
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
                            let color =
                                decoration_fill_color(decoration, *stroke_color, *background_color);
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

    fn image_resources(&self) -> Result<Vec<PdfImageResource>, FormatEncodeError> {
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(self.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_image_resources",
            })?;
        for element in &self.elements {
            let PdfElement::Image {
                asset,
                dark_mode_filter,
                ..
            } = element
            else {
                continue;
            };
            if asset.sanitized_svg.is_some() {
                continue;
            }
            if resources
                .iter()
                .any(|candidate: &PdfImageResource| candidate.asset.identity == asset.identity)
            {
                continue;
            }
            resources.push(PdfImageResource {
                asset: Arc::clone(asset),
                dark_mode_filter: *dark_mode_filter,
            });
        }
        resources.sort_unstable_by_key(|resource| resource.asset.identity);
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
                    Arc::ptr_eq(&candidate.asset, asset)
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
                asset: Arc::clone(asset),
                raster_scale,
            });
        }
        resources.sort_by_key(|resource| resource.asset.identity);
        Ok(resources)
    }
}

impl PdfElement {
    fn link_geometry(&self) -> Option<(&str, f64, f64, &ResolvedTransform, &[ResolvedClip])> {
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
    transform: &ResolvedTransform,
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
    transform: &ResolvedTransform,
    frame_clips: &[ResolvedClip],
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
    transform: &ResolvedTransform,
    frame_clips: &[ResolvedClip],
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
    clip: &ResolvedClip,
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
    let mut current = Point { x: 0.0, y: 0.0 };
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
                    let point = Point {
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
    point: Point,
    transform: &ResolvedTransform,
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

/// The rectangle of the element's space the reference leaves visible, as
/// `(x, y, width, height)` after the flip: the `<use>` viewport as the flip
/// mirrors it about the element's unrounded center, intersected with the crop
/// `<mask>`, which sits on the unflipped group, when the image is cropped.
fn visible_rect(
    projection: &ResolvedImageProjection,
    width: f64,
    height: f64,
    scale: [i8; 2],
) -> (f64, f64, f64, f64) {
    let (x, y, viewport_width, viewport_height) = projection.viewport_rect();
    let mut left = if scale[0] < 0 {
        width - x - viewport_width
    } else {
        x
    };
    let mut top = if scale[1] < 0 {
        height - y - viewport_height
    } else {
        y
    };
    let mut right = left + viewport_width;
    let mut bottom = top + viewport_height;
    if projection.crop.is_some() {
        left = left.max(0.0);
        top = top.max(0.0);
        right = right.min(projection.viewport_width);
        bottom = bottom.min(projection.viewport_height);
    }
    (left, top, right - left, bottom - top)
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
