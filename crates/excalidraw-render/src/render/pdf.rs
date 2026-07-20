use std::fmt::Write as _;
use std::io::Write as _;

use crate::RenderRequest;

use super::{
    rough::{self, Path, PathOperation},
    FormatEncodeError, NormalizedElementSource, ResolvedElement, ResolvedScene,
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
// broader PDF operation qualification remains outside this rectangle slice.
pub(super) struct PdfPlan {
    logical_width: f64,
    logical_height: f64,
    scale: f64,
    page_width: f64,
    page_height: f64,
    background: Option<[f64; 3]>,
    elements: Vec<PdfElement>,
}

enum PdfElement {
    RoughRectangle {
        source: NormalizedElementSource,
        path: Path,
        transform: super::ResolvedTransform,
        stroke_color: [f64; 3],
        stroke_width: f64,
        opacity: f64,
    },
}

impl PdfPlan {
    pub(super) fn new(
        scene: ResolvedScene,
        request: &RenderRequest,
    ) -> Result<Self, FormatEncodeError> {
        let scale = scene.raster_or_pdf_scale(request.sizing());
        let (page_width, page_height) = page_dimensions(&scene, request)?;
        let background = scene
            .include_background
            .then(|| parse_srgb(&scene.background_color))
            .transpose()?;
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(scene.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_plan",
                subject: "pdf_elements",
            })?;
        for element in scene.elements {
            match element {
                ResolvedElement::Rectangle(rectangle) => {
                    if rectangle.source.id().is_empty() {
                        return Err(FormatEncodeError::Encoding(format!(
                            "rectangle at {} has no normalized identity",
                            rectangle.source.pointer()
                        )));
                    }
                    elements.push(PdfElement::RoughRectangle {
                        source: rectangle.source,
                        path: rectangle.path,
                        transform: rectangle.transform,
                        stroke_color: parse_srgb(&rectangle.stroke_color)?,
                        stroke_width: rectangle.stroke_width,
                        opacity: rectangle.opacity,
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
            elements,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        let opacity_resources = self.opacity_resources()?;
        let content = self.encode_content(&opacity_resources)?;
        let srgb_icc_profile = srgb_icc_profile()?;
        let object_count = 6_usize
            .checked_add(opacity_resources.len())
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
            "<<\n/Type /Catalog\n/Pages 2 0 R\n/OutputIntents [{OUTPUT_INTENT_OBJECT} 0 R]\n>>\nendobj\n"
        )
        .map_err(write_error)?;

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
        pdf.extend_from_slice(b">>\n/Contents 4 0 R\n>>\nendobj\n");

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

    pub(super) fn source(&self, index: usize) -> Option<(&str, &str)> {
        self.elements.get(index).map(|element| match element {
            PdfElement::RoughRectangle { source, .. } => (source.id(), source.pointer()),
        })
    }

    fn opacity_resources(&self) -> Result<Vec<f64>, FormatEncodeError> {
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(self.elements.len())
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "format_workspace",
                subject: "pdf_opacity_resources",
            })?;
        for opacity in self.elements.iter().filter_map(|element| match element {
            PdfElement::RoughRectangle { opacity, .. } if *opacity != 1.0 => Some(*opacity),
            _ => None,
        }) {
            if !resources
                .iter()
                .any(|retained: &f64| retained.to_bits() == opacity.to_bits())
            {
                resources.push(opacity);
            }
        }
        Ok(resources)
    }

    fn encode_content(&self, opacity_resources: &[f64]) -> Result<String, FormatEncodeError> {
        let mut capacity = 1_024_usize;
        for element in &self.elements {
            let PdfElement::RoughRectangle { path, .. } = element;
            capacity = path
                .operations
                .len()
                .checked_mul(200)
                .and_then(|path_bytes| path_bytes.checked_add(512))
                .and_then(|element_bytes| capacity.checked_add(element_bytes))
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
            content.push_str("/CS1 cs\n");
            push_color(&mut content, background, "scn")?;
            push_pdf_number(&mut content, 0.0, "PDF background x")?;
            content.push(' ');
            push_pdf_number(&mut content, 0.0, "PDF background y")?;
            content.push(' ');
            push_pdf_number(&mut content, self.logical_width, "PDF background width")?;
            content.push(' ');
            push_pdf_number(&mut content, self.logical_height, "PDF background height")?;
            content.push_str(" re\nf\n");
        }
        for element in &self.elements {
            match element {
                PdfElement::RoughRectangle {
                    source,
                    path,
                    transform,
                    stroke_color,
                    stroke_width,
                    opacity,
                } => {
                    if source.id().is_empty() || source.pointer().is_empty() {
                        return Err(FormatEncodeError::Encoding(
                            "PDF plan rectangle lost its source identity".to_owned(),
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
                    if *opacity != 1.0 {
                        let resource = opacity_resources
                            .iter()
                            .position(|candidate| candidate.to_bits() == opacity.to_bits())
                            .ok_or_else(|| {
                                FormatEncodeError::Encoding(
                                    "PDF opacity resource has no owner".to_owned(),
                                )
                            })?;
                        writeln!(content, "/GS{} gs", resource + 1).map_err(|_| {
                            FormatEncodeError::Encoding(
                                "failed to write PDF opacity resource".to_owned(),
                            )
                        })?;
                    }
                    push_pdf_number(&mut content, *stroke_width, "PDF stroke width")?;
                    content.push_str(" w\n1 J\n/CS1 CS\n");
                    push_color(&mut content, *stroke_color, "SCN")?;
                    push_rough_path(&mut content, path)?;
                    content.push_str("S\nQ\n");
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

fn parse_srgb(color: &str) -> Result<[f64; 3], FormatEncodeError> {
    let bytes = color.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' {
        return Err(FormatEncodeError::Encoding(format!(
            "PDF solid color is not canonical sRGB: {color}"
        )));
    }
    Ok([
        f64::from(parse_hex_byte(bytes[1], bytes[2])?) / 255.0,
        f64::from(parse_hex_byte(bytes[3], bytes[4])?) / 255.0,
        f64::from(parse_hex_byte(bytes[5], bytes[6])?) / 255.0,
    ])
}

fn parse_hex_byte(high: u8, low: u8) -> Result<u8, FormatEncodeError> {
    let digit = |value| match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    };
    let high = digit(high).ok_or_else(|| {
        FormatEncodeError::Encoding("PDF solid color contains an invalid digit".to_owned())
    })?;
    let low = digit(low).ok_or_else(|| {
        FormatEncodeError::Encoding("PDF solid color contains an invalid digit".to_owned())
    })?;
    Ok(high * 16 + low)
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
