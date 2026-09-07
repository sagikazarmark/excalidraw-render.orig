//! Shared inspection of the renderer's PDF text output for the companion tests
//! of font-bearing conformance cases: which programs a PDF embeds, which page
//! resource a face is drawn through, what its ToUnicode CMap declares, and the
//! logical text its `ActualText` carries. Each test binary includes this module
//! by path and reads the PDF as the bytes the renderer wrote, so what is pinned
//! here is the artifact, not a viewer's interpretation of it.

// Each companion test includes this module by path and uses the part it needs.
#![allow(dead_code)]

/// The body of the ToUnicode CMap stream that declares `cmap_name`.
pub fn to_unicode_cmap<'a>(pdf: &'a str, cmap_name: &str) -> &'a str {
    pdf.split(cmap_name)
        .nth(1)
        .and_then(|rest| rest.split("endcmap").next())
        .unwrap_or_else(|| panic!("the PDF carries the ToUnicode CMap {cmap_name}"))
}

/// The English family name (name ID 1) of an embedded face.
pub fn family_name(face: &rustybuzz::Face<'_>) -> String {
    face.names()
        .into_iter()
        .find(|name| name.name_id == 1 && name.language_id == 0x0409)
        .and_then(|name| name.to_string())
        .expect("embedded face keeps its English family name")
}

/// The page resource name (`/F<n>`) of the Type0 font with the given base font.
pub fn font_resource(pdf: &str, base_font: &str) -> String {
    let header = format!("<<\n/Type /Font\n/Subtype /Type0\n{base_font}\n");
    let position = pdf.find(&header).expect("the base font has a Type0 object");
    let object = pdf[..position]
        .strip_suffix(" 0 obj\n")
        .and_then(|head| head.rsplit('\n').next())
        .expect("the Type0 font is an indirect object");
    let fonts = pdf
        .split("/Font << ")
        .nth(1)
        .and_then(|rest| rest.split(" >>").next())
        .expect("the page declares a font resource dictionary");
    let reference = format!(" {object} 0 R");
    let position = fonts
        .find(&reference)
        .expect("the page resources reference the font");
    fonts[..position]
        .rsplit(' ')
        .next()
        .expect("the reference has a resource name")
        .to_owned()
}

/// The marked-content body that follows `actual_text`, up to its `EMC`: the
/// text-showing operators of one line.
pub fn marked_content_line<'a>(pdf: &'a str, actual_text: &str) -> &'a str {
    pdf.split(actual_text)
        .nth(1)
        .and_then(|rest| rest.split("EMC").next())
        .expect("the stored text opens one marked-content line")
}

/// Every `ActualText` operand decoded from UTF-16BE, concatenated in document
/// order: the logical text a PDF reader extracts.
pub fn pdf_actual_text(pdf: &str) -> String {
    let mut extracted = String::new();
    for tail in pdf.split("/ActualText <FEFF").skip(1) {
        let hex = tail.split('>').next().expect("ActualText hex terminates");
        let units = hex
            .as_bytes()
            .chunks_exact(4)
            .map(|digits| {
                u16::from_str_radix(std::str::from_utf8(digits).expect("hex is ASCII"), 16)
                    .expect("ActualText is hexadecimal")
            })
            .collect::<Vec<_>>();
        extracted.push_str(&String::from_utf16(&units).expect("ActualText is UTF-16"));
    }
    extracted
}

/// Every embedded `FontFile2` stream: the PDF writes them uncompressed with
/// `/Length1` naming the complete sfnt length.
pub fn embedded_pdf_fonts(pdf: &[u8]) -> Vec<Vec<u8>> {
    const MARKER: &[u8] = b"/Length1 ";
    const STREAM: &[u8] = b">>\nstream\n";
    let mut fonts = Vec::new();
    let mut offset = 0;
    while let Some(position) = pdf[offset..]
        .windows(MARKER.len())
        .position(|window| window == MARKER)
    {
        let start = offset + position + MARKER.len();
        let digits = pdf[start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        let length = std::str::from_utf8(&pdf[start..start + digits])
            .expect("font length is ASCII")
            .parse::<usize>()
            .expect("font length is numeric");
        let stream_start = start + digits + 1 + STREAM.len();
        assert_eq!(&pdf[start + digits + 1..stream_start], STREAM);
        fonts.push(pdf[stream_start..stream_start + length].to_vec());
        offset = stream_start + length;
    }
    fonts
}
