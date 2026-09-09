//! Shared inspection of the renderer's PDF text output for the companion tests
//! of font-bearing conformance cases: which programs a PDF embeds, which page
//! resource a face is drawn through, what its ToUnicode CMap declares, and the
//! logical text its `ActualText` carries. Each test binary includes this module
//! by path and reads the PDF as the bytes the renderer wrote, so what is pinned
//! here is the artifact, not a viewer's interpretation of it.
//!
//! Every stream the renderer writes is `/Filter /FlateDecode` (ADR-0008), so a
//! test that reads operators, font programs or CMaps goes through
//! [`inflate_streams`] (or [`pdf_text`]) first: the artifact with each Flate
//! stream inflated in place and its filter entry dropped, byte-identical to the
//! artifact everywhere else.

/// The PDF with every `/Filter /FlateDecode` stream body inflated in place:
/// the `/Filter` line is dropped, `/Length` is rewritten to the inflated
/// length, and every other byte of the artifact is kept, so the dictionaries,
/// object numbers and `/Length1` a test reads are the artifact's own.
pub fn inflate_streams(pdf: &[u8]) -> Vec<u8> {
    const FILTER_LINE: &[u8] = b"/Filter /FlateDecode\n";
    const LENGTH: &[u8] = b"/Length ";
    const OBJECT: &[u8] = b" 0 obj\n";
    const STREAM: &[u8] = b"stream\n";
    const ENDSTREAM: &[u8] = b"\nendstream";
    let mut inflated = Vec::with_capacity(pdf.len() * 2);
    let mut cursor = 0;
    while let Some(relative) = find_bytes(&pdf[cursor..], STREAM) {
        let stream_start = cursor + relative + STREAM.len();
        // The stream dictionary runs from the object header to the keyword.
        let dictionary_start = pdf[cursor..stream_start]
            .windows(OBJECT.len())
            .rposition(|window| window == OBJECT)
            .map_or(cursor, |position| cursor + position);
        let dictionary = &pdf[dictionary_start..stream_start];
        let length_at = find_bytes(dictionary, LENGTH).expect("a stream declares /Length");
        let digits_start = length_at + LENGTH.len();
        let length_end = digits_start
            + dictionary[digits_start..]
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
        let length = std::str::from_utf8(&dictionary[digits_start..length_end])
            .expect("length is ASCII")
            .parse::<usize>()
            .expect("length is numeric");
        let body_end = stream_start + length;
        assert_eq!(
            &pdf[body_end..body_end + ENDSTREAM.len()],
            ENDSTREAM,
            "the declared /Length ends at endstream"
        );
        let Some(filter) = find_bytes(dictionary, FILTER_LINE) else {
            inflated.extend_from_slice(&pdf[cursor..body_end + ENDSTREAM.len()]);
            cursor = body_end + ENDSTREAM.len();
            continue;
        };
        let data = miniz_oxide::inflate::decompress_to_vec_zlib(&pdf[stream_start..body_end])
            .expect("a FlateDecode stream inflates as zlib");
        // Rewrite the dictionary: drop the filter line, restate the length.
        let mut edits = [
            (filter, filter + FILTER_LINE.len(), Vec::new()),
            (
                digits_start,
                length_end,
                data.len().to_string().into_bytes(),
            ),
        ];
        edits.sort_by_key(|edit| edit.0);
        inflated.extend_from_slice(&pdf[cursor..dictionary_start]);
        let mut copied = 0;
        for (start, end, replacement) in edits {
            inflated.extend_from_slice(&dictionary[copied..start]);
            inflated.extend_from_slice(&replacement);
            copied = end;
        }
        inflated.extend_from_slice(&dictionary[copied..]);
        inflated.extend_from_slice(&data);
        inflated.extend_from_slice(ENDSTREAM);
        cursor = body_end + ENDSTREAM.len();
    }
    inflated.extend_from_slice(&pdf[cursor..]);
    inflated
}

/// The artifact as text with every Flate stream inflated: what a test reads
/// when it looks for operators, dictionary entries or CMap lines.
pub fn pdf_text(pdf: &[u8]) -> String {
    String::from_utf8_lossy(&inflate_streams(pdf)).into_owned()
}

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

/// The page resource name (`/F<n>`) of the Type0 font whose base font is
/// `pdf_name`, with or without its subset tag.
pub fn font_resource(pdf: &str, pdf_name: &str) -> String {
    let header = format!(
        "<<\n/Type /Font\n/Subtype /Type0\n/BaseFont /{}\n",
        base_font(pdf, pdf_name)
    );
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

/// Every embedded `FontFile2` stream, inflated: `/Length1` names the complete
/// sfnt length, which is the inflated body's.
pub fn embedded_pdf_fonts(pdf: &[u8]) -> Vec<Vec<u8>> {
    const MARKER: &[u8] = b"/Length1 ";
    const STREAM: &[u8] = b">>\nstream\n";
    let pdf = inflate_streams(pdf);
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

/// The inflated bytes of the stream that `object` (for example `b"4 0 obj"`)
/// opens.
pub fn stream_data(pdf: &[u8], object: &[u8]) -> Vec<u8> {
    let pdf = inflate_streams(pdf);
    let object_start = find_bytes(&pdf, object).expect("expected PDF object exists");
    let marker = b"stream\n";
    let stream_start = object_start
        + find_bytes(&pdf[object_start..], marker).expect("object has a stream")
        + marker.len();
    let stream_end = stream_start
        + find_bytes(&pdf[stream_start..], b"\nendstream").expect("stream has an end marker");
    pdf[stream_start..stream_end].to_vec()
}

/// The inflated stream that `object` opens, as the text a content stream or
/// CMap is.
pub fn stream_text(pdf: &[u8], object: &[u8]) -> String {
    String::from_utf8(stream_data(pdf, object)).expect("stream is UTF-8 text")
}

/// Every `/BaseFont` name the PDF's Type0 fonts declare, in document order,
/// subset tag included: `OEECDL+Excalifont` for an official font chunk's
/// program, `NotoEmoji` for the complete face.
pub fn type0_base_fonts(pdf: &str) -> Vec<&str> {
    pdf.split("/Type /Font\n/Subtype /Type0\n/BaseFont /")
        .skip(1)
        .map(|tail| tail.split('\n').next().expect("the base font line ends"))
        .collect()
}

/// The program name that spells `pdf_name` once its subset tag is removed:
/// six uppercase letters and `+` for an official font chunk (ADR-0008), or
/// the bare name of a complete face.
pub fn strip_subset_tag(base_font: &str) -> &str {
    match base_font.split_once('+') {
        Some((tag, name))
            if tag.len() == 6 && tag.bytes().all(|byte| (b'A'..=b'P').contains(&byte)) =>
        {
            name
        }
        _ => base_font,
    }
}

/// Whether the PDF declares a Type0 font for `pdf_name`, tagged or not.
pub fn declares_base_font(pdf: &str, pdf_name: &str) -> bool {
    type0_base_fonts(pdf)
        .into_iter()
        .any(|base_font| strip_subset_tag(base_font) == pdf_name)
}

/// The declared `/BaseFont` of `pdf_name`'s first Type0 font, tag included.
pub fn base_font<'a>(pdf: &'a str, pdf_name: &str) -> &'a str {
    type0_base_fonts(pdf)
        .into_iter()
        .find(|base_font| strip_subset_tag(base_font) == pdf_name)
        .unwrap_or_else(|| panic!("the PDF declares a Type0 font for {pdf_name}"))
}

/// The two equal file identifiers of the trailer `/ID`, as the PDF spells
/// them: 32 uppercase hex digits each (ADR-0008).
pub fn trailer_file_identifiers(pdf: &[u8]) -> (String, String) {
    let text = String::from_utf8_lossy(pdf);
    let trailer = text
        .rsplit("\ntrailer\n")
        .next()
        .expect("the PDF ends in a trailer");
    let ids = trailer
        .split("/ID [<")
        .nth(1)
        .and_then(|rest| rest.split(">]").next())
        .expect("the trailer carries /ID");
    let (first, second) = ids.split_once("> <").expect("/ID carries two strings");
    (first.to_owned(), second.to_owned())
}

/// The offset of the first `needle` in `haystack`.
pub fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
