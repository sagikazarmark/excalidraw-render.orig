use std::fmt::Write as _;
use std::io::Write as _;

use crate::{color::CanonicalColor, RenderRequest};

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

// This focused direct serializer is the controlled backend approved for #44;
// broader PDF operation qualification remains outside this outline slice.
pub(super) struct PdfPlan {
    logical_width: f64,
    logical_height: f64,
    scale: f64,
    page_width: f64,
    page_height: f64,
    background: Option<CanonicalColor>,
    fallback_colors: super::FallbackColors,
    mandatory_manifest: Option<String>,
    fallback_font_sfnt: Option<Vec<u8>>,
    elements: Vec<PdfElement>,
}

enum PdfElement {
    RoughOutline {
        source: NormalizedElementSource,
        drawing: Drawing,
        transform: super::ResolvedTransform,
        stroke_color: CanonicalColor,
        fill_color: Option<CanonicalColor>,
        stroke_width: f64,
        stroke_style: StrokeStyle,
        source_stroke_width: f64,
        opacity: f64,
    },
    Fallback {
        source: super::FallbackSource,
        label: Option<super::text::ResolvedLabel>,
        link: Option<String>,
        frame_clip: Option<super::ResolvedClip>,
        width: f64,
        height: f64,
        transform: super::ResolvedTransform,
        opacity: f64,
    },
}

struct PdfGlyphMapping {
    glyph_id: u16,
    advance_1000: u16,
    unicode: Vec<u16>,
}

impl PdfPlan {
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
                    elements.push(PdfElement::RoughOutline {
                        source: outline.source,
                        drawing: outline.drawing,
                        transform: outline.transform,
                        stroke_color: outline.stroke_color,
                        fill_color: outline.fill_color,
                        stroke_width: outline.stroke_width,
                        stroke_style: outline.stroke_style,
                        source_stroke_width: outline.source_stroke_width,
                        opacity: outline.opacity,
                    });
                }
                ResolvedElement::Fallback(fallback) => {
                    elements.push(PdfElement::Fallback {
                        source: fallback.source,
                        label: fallback.label,
                        link: fallback.link,
                        frame_clip: fallback.frame_clip,
                        width: fallback.width,
                        height: fallback.height,
                        transform: fallback.transform,
                        opacity: fallback.opacity,
                    });
                }
            }
        }
        Ok(Self {
            logical_width: scene.logical_width,
            logical_height: scene.logical_height,
            scale,
            page_width,
            page_height,
            background,
            fallback_colors: scene.fallback_colors,
            mandatory_manifest: scene.mandatory_manifest,
            fallback_font_sfnt: scene.fallback_font_sfnt,
            elements,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let opacity_resources = self.opacity_resources()?;
        let content = self.encode_content(&opacity_resources)?;
        let fallback_forms = self.encode_fallback_forms()?;
        let xmp = self
            .mandatory_manifest
            .as_deref()
            .map(xmp_metadata)
            .transpose()?;
        let link_count = self
            .elements
            .iter()
            .filter(|element| match element {
                PdfElement::Fallback {
                    link: Some(_),
                    width,
                    height,
                    transform,
                    frame_clip,
                    ..
                } => link_annotation_visible(
                    *width,
                    *height,
                    transform,
                    frame_clip.as_ref(),
                    self.scale * POINTS_PER_LOGICAL_PIXEL,
                    self.page_width,
                    self.page_height,
                ),
                _ => false,
            })
            .count();
        let srgb_icc_profile = srgb_icc_profile()?;
        let object_count = 6_usize
            .checked_add(opacity_resources.len())
            .and_then(|count| count.checked_add(usize::from(xmp.is_some())))
            .and_then(|count| count.checked_add(link_count))
            .and_then(|count| count.checked_add(fallback_forms.len()))
            .ok_or_else(pdf_size_error)?;
        let metadata_object = xmp
            .as_ref()
            .map(|_| FIRST_EXT_GSTATE_OBJECT + opacity_resources.len());
        let first_link_object = FIRST_EXT_GSTATE_OBJECT
            .checked_add(opacity_resources.len())
            .and_then(|object| object.checked_add(usize::from(xmp.is_some())))
            .ok_or_else(pdf_size_error)?;
        let first_fallback_form_object = first_link_object
            .checked_add(link_count)
            .ok_or_else(pdf_size_error)?;
        let fallback_font_object = self
            .fallback_font_sfnt
            .as_ref()
            .map(|_| first_fallback_form_object + fallback_forms.len());
        let fallback_font_object_count = usize::from(fallback_font_object.is_some()) * 5;
        let object_count = object_count
            .checked_add(fallback_font_object_count)
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
                xmp.as_ref()
                    .map_or(Some(capacity), |xmp| capacity.checked_add(xmp.len() + 128))
            })
            .and_then(|capacity| {
                link_count
                    .checked_mul(512)
                    .and_then(|extra| capacity.checked_add(extra))
            })
            .and_then(|capacity| {
                fallback_forms.iter().try_fold(capacity, |capacity, form| {
                    capacity.checked_add(form.len().saturating_add(256))
                })
            })
            .and_then(|capacity| {
                self.fallback_font_sfnt
                    .as_ref()
                    .map_or(Some(capacity), |font| {
                        capacity.checked_add(font.len().saturating_add(16_384))
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
                    FIRST_EXT_GSTATE_OBJECT + index
                )
                .map_err(write_error)?;
            }
            pdf.extend_from_slice(b" >>\n");
        }
        if !fallback_forms.is_empty() {
            pdf.extend_from_slice(b"/XObject <<");
            for index in 0..fallback_forms.len() {
                write!(
                    pdf,
                    " /FB{} {} 0 R",
                    index + 1,
                    first_fallback_form_object + index
                )
                .map_err(write_error)?;
            }
            pdf.extend_from_slice(b" >>\n");
        }
        if let Some(fallback_font_object) = fallback_font_object {
            writeln!(pdf, "/Font << /F1 {fallback_font_object} 0 R >>").map_err(write_error)?;
        }
        pdf.extend_from_slice(b">>\n");
        if link_count != 0 {
            pdf.extend_from_slice(b"/Annots [");
            for index in 0..link_count {
                write!(pdf, " {} 0 R", first_link_object + index).map_err(write_error)?;
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
            begin_object(&mut pdf, &mut offsets, FIRST_EXT_GSTATE_OBJECT + index)?;
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
        let mut link_index = 0_usize;
        for element in &self.elements {
            let PdfElement::Fallback {
                link: Some(link),
                width,
                height,
                transform,
                frame_clip,
                ..
            } = element
            else {
                continue;
            };
            if !link_annotation_visible(
                *width,
                *height,
                transform,
                frame_clip.as_ref(),
                self.scale * POINTS_PER_LOGICAL_PIXEL,
                self.page_width,
                self.page_height,
            ) {
                continue;
            }
            let object = first_link_object
                .checked_add(link_index)
                .ok_or_else(pdf_size_error)?;
            begin_object(&mut pdf, &mut offsets, object)?;
            self.write_link_annotation(
                &mut pdf,
                link,
                *width,
                *height,
                transform,
                frame_clip.as_ref(),
            )?;
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
            self.write_fallback_font_objects(&mut pdf, &mut offsets, font_object, font)?;
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
        frame_clip: Option<&super::ResolvedClip>,
    ) -> Result<(), FormatEncodeError> {
        let sin = transform.angle_radians.sin();
        let cos = transform.angle_radians.cos();
        let offset_x = transform.translation_x + transform.center_x - cos * transform.center_x
            + sin * transform.center_y;
        let offset_y = transform.translation_y + transform.center_y
            - sin * transform.center_x
            - cos * transform.center_y;
        let point_scale = self.scale * POINTS_PER_LOGICAL_PIXEL;
        let map = |x: f64, y: f64| {
            let logical_x = cos * x - sin * y + offset_x;
            let logical_y = sin * x + cos * y + offset_y;
            (
                logical_x * point_scale,
                self.page_height - logical_y * point_scale,
            )
        };
        let top_left = map(0.0, 0.0);
        let top_right = map(width, 0.0);
        let bottom_left = map(0.0, height);
        let bottom_right = map(width, height);
        let mut points = [top_left, top_right, bottom_left, bottom_right];
        let mut clip_min_x = 0.0;
        let mut clip_min_y = 0.0;
        let mut clip_max_x = self.page_width;
        let mut clip_max_y = self.page_height;
        if let Some(frame_clip) = frame_clip {
            let frame_points = transformed_pdf_points(
                frame_clip.width,
                frame_clip.height,
                &frame_clip.transform,
                point_scale,
                self.page_height,
            );
            clip_min_x = frame_points
                .iter()
                .map(|point| point.0)
                .fold(f64::INFINITY, f64::min)
                .max(clip_min_x);
            clip_min_y = frame_points
                .iter()
                .map(|point| point.1)
                .fold(f64::INFINITY, f64::min)
                .max(clip_min_y);
            clip_max_x = frame_points
                .iter()
                .map(|point| point.0)
                .fold(f64::NEG_INFINITY, f64::max)
                .min(clip_max_x);
            clip_max_y = frame_points
                .iter()
                .map(|point| point.1)
                .fold(f64::NEG_INFINITY, f64::max)
                .min(clip_max_y);
        }
        for point in &mut points {
            point.0 = point.0.clamp(clip_min_x, clip_max_x);
            point.1 = point.1.clamp(clip_min_y, clip_max_y);
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
        for (index, (x, y)) in points.into_iter().enumerate() {
            if index != 0 {
                pdf.push(b' ');
            }
            push_pdf_vec_number(pdf, x, "PDF link quadrilateral x")?;
            pdf.push(b' ');
            push_pdf_vec_number(pdf, y, "PDF link quadrilateral y")?;
        }
        pdf.extend_from_slice(b"]\n/Border [0 0 0]\n/A <<\n/S /URI\n/URI (");
        push_pdf_link(pdf, link);
        pdf.extend_from_slice(b")\n>>\n>>\nendobj\n");
        Ok(())
    }

    fn write_fallback_font_objects(
        &self,
        pdf: &mut Vec<u8>,
        offsets: &mut [usize],
        font_object: usize,
        font: &[u8],
    ) -> Result<(), FormatEncodeError> {
        let cid_font_object = font_object + 1;
        let descriptor_object = font_object + 2;
        let font_file_object = font_object + 3;
        let to_unicode_object = font_object + 4;
        let mappings = self.fallback_glyph_mappings()?;
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
                "<<\n/Type /Font\n/Subtype /Type0\n/BaseFont /LiberationSans\n",
                "/Encoding /Identity-H\n/DescendantFonts [{} 0 R]\n",
                "/ToUnicode {} 0 R\n>>\nendobj\n"
            ),
            cid_font_object, to_unicode_object,
        )
        .map_err(write_error)?;

        begin_object(pdf, offsets, cid_font_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /Font\n/Subtype /CIDFontType2\n/BaseFont /LiberationSans\n",
                "/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>\n",
                "/FontDescriptor {} 0 R\n/CIDToGIDMap /Identity\n/DW 1000\n/W ["
            ),
            descriptor_object,
        )
        .map_err(write_error)?;
        for mapping in &mappings {
            write!(pdf, " {} [{}]", mapping.glyph_id, mapping.advance_1000).map_err(write_error)?;
        }
        pdf.extend_from_slice(b" ]\n>>\nendobj\n");

        begin_object(pdf, offsets, descriptor_object)?;
        write!(
            pdf,
            concat!(
                "<<\n/Type /FontDescriptor\n/FontName /LiberationSans\n/Flags 32\n",
                "/FontBBox [{} {} {} {}]\n/ItalicAngle 0\n/Ascent {}\n/Descent {}\n",
                "/CapHeight {}\n/StemV 80\n/FontFile2 {} 0 R\n>>\nendobj\n"
            ),
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

        let cmap = fallback_to_unicode_cmap(&mappings)?;
        begin_object(pdf, offsets, to_unicode_object)?;
        write!(pdf, "<<\n/Length {}\n>>\nstream\n", cmap.len()).map_err(write_error)?;
        pdf.extend_from_slice(cmap.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        Ok(())
    }

    fn fallback_glyph_mappings(&self) -> Result<Vec<PdfGlyphMapping>, FormatEncodeError> {
        let mut mappings = Vec::new();
        for label in self.elements.iter().filter_map(|element| match element {
            PdfElement::Fallback {
                label: Some(label), ..
            } => Some(label),
            _ => None,
        }) {
            for glyph in &label.glyphs {
                if mappings
                    .iter()
                    .any(|mapping: &PdfGlyphMapping| mapping.glyph_id == glyph.id)
                {
                    continue;
                }
                let mut unicode = Vec::new();
                unicode
                    .try_reserve_exact(glyph.unicode.len())
                    .map_err(|_| FormatEncodeError::MemoryUnavailable {
                        subject_kind: "format_workspace",
                        subject: "pdf_fallback_to_unicode",
                    })?;
                unicode.extend_from_slice(&glyph.unicode);
                mappings
                    .try_reserve(1)
                    .map_err(|_| FormatEncodeError::MemoryUnavailable {
                        subject_kind: "format_workspace",
                        subject: "pdf_fallback_glyph_mappings",
                    })?;
                mappings.push(PdfGlyphMapping {
                    glyph_id: glyph.id,
                    advance_1000: glyph.advance_1000,
                    unicode,
                });
            }
        }
        mappings.sort_unstable_by_key(|mapping| mapping.glyph_id);
        Ok(mappings)
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
                    ..
                } => {
                    retain(*opacity * stroke_color.alpha());
                    if let Some(fill_color) = fill_color {
                        retain(*opacity * fill_color.alpha());
                    }
                }
                PdfElement::Fallback { opacity, .. } => retain(*opacity),
            }
        }
        Ok(resources)
    }

    fn encode_fallback_forms(&self) -> Result<Vec<String>, FormatEncodeError> {
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
            )?;
            forms.push(form);
        }
        Ok(forms)
    }

    fn encode_content(&self, opacity_resources: &[f64]) -> Result<String, FormatEncodeError> {
        let mut capacity = 1_024_usize;
        for element in &self.elements {
            let element_bytes = match element {
                PdfElement::RoughOutline { drawing, .. } => drawing
                    .operation_count()
                    .checked_mul(200)
                    .and_then(|path_bytes| path_bytes.checked_add(512))
                    .ok_or_else(pdf_size_error)?,
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
                    transform,
                    stroke_color,
                    fill_color,
                    stroke_width,
                    stroke_style,
                    source_stroke_width,
                    opacity,
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
                    let opacity = *opacity * stroke_color.alpha();
                    if opacity != 1.0 {
                        push_opacity_resource(&mut content, opacity_resources, opacity)?;
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
                    content.push_str("S\nQ\n");
                }
                PdfElement::Fallback {
                    source,
                    label,
                    link: _,
                    frame_clip,
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
                    if let Some(frame_clip) = frame_clip {
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
                    )?;
                    content.push_str("Q\n");
                }
            }
        }
        content.push('Q');
        Ok(content)
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
    frame_clip: Option<&super::ResolvedClip>,
    point_scale: f64,
    page_width: f64,
    page_height: f64,
) -> bool {
    let bounds = |points: &[(f64, f64); 4]| {
        (
            points
                .iter()
                .map(|point| point.0)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.1)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.0)
                .fold(f64::NEG_INFINITY, f64::max),
            points
                .iter()
                .map(|point| point.1)
                .fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let element = transformed_pdf_points(width, height, transform, point_scale, page_height);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = bounds(&element);
    min_x = min_x.max(0.0);
    min_y = min_y.max(0.0);
    max_x = max_x.min(page_width);
    max_y = max_y.min(page_height);
    if let Some(frame_clip) = frame_clip {
        let frame = transformed_pdf_points(
            frame_clip.width,
            frame_clip.height,
            &frame_clip.transform,
            point_scale,
            page_height,
        );
        let (frame_min_x, frame_min_y, frame_max_x, frame_max_y) = bounds(&frame);
        min_x = min_x.max(frame_min_x);
        min_y = min_y.max(frame_min_y);
        max_x = max_x.min(frame_max_x);
        max_y = max_y.min(frame_max_y);
    }
    min_x < max_x && min_y < max_y
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
        push_pdf_label_text(output, label, colors.label)?;
        output.push_str("EMC\n");
    }
    Ok(())
}

fn push_pdf_label_text(
    output: &mut String,
    label: &super::text::ResolvedLabel,
    color: CanonicalColor,
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
        write!(output, "{:04X}", glyph.id).map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF fallback glyph ID".to_owned())
        })?;
        output.push_str("> Tj\n");
    }
    output.push_str("ET\n");
    Ok(())
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
    let corners = [
        map(0.0, 0.0),
        map(clip.width, 0.0),
        map(clip.width, clip.height),
        map(0.0, clip.height),
    ];
    for (index, (x, y)) in corners.into_iter().enumerate() {
        push_pdf_point(output, x, y, if index == 0 { "m" } else { "l" })?;
    }
    output.push_str("h\nW\nn\n");
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

fn fallback_to_unicode_cmap(mappings: &[PdfGlyphMapping]) -> Result<String, FormatEncodeError> {
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
        "/CMapName /LiberationSans-UCS def\n/CMapType 2 def\n",
        "1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n"
    ));
    for chunk in mappings.chunks(100) {
        writeln!(cmap, "{} beginbfchar", chunk.len()).map_err(|_| {
            FormatEncodeError::Encoding("failed to write PDF ToUnicode count".to_owned())
        })?;
        for mapping in chunk {
            write!(cmap, "<{:04X}> <", mapping.glyph_id).map_err(|_| {
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
