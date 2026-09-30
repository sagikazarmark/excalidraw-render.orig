//! Shared inspection of the renderer's PDF text output for the companion tests
//! of font-bearing conformance cases: which programs a PDF embeds, which page
//! resource a face is drawn through, what its ToUnicode CMap declares, and the
//! logical text its `ActualText` carries. Each test binary includes this module
//! by path and reads the PDF as the bytes the renderer wrote, so what is pinned
//! here is the artifact, not a viewer's interpretation of it.
//!
//! The renderer writes PDF through Krilla (ADR-0021), whose syntax is
//! compact. [`inflate_streams`] (and [`pdf_text`]) therefore read the artifact
//! into one normalized layout before a test looks at it: every object parsed,
//! each dictionary entry on its own line, arrays and nested dictionaries
//! spaced, every Flate stream inflated with its `/Filter` dropped and its
//! `/Length` restated, and every content stream one operator per line with
//! `/ActualText` spelled as UTF-16BE hex. The values are the artifact's own;
//! only the whitespace and the stream encoding differ, so what is pinned here
//! is still the artifact, not a viewer's interpretation of it.

use std::collections::BTreeSet;

#[derive(Clone, Debug)]
enum Value {
    Name(Vec<u8>),
    Atom(Vec<u8>),
    Literal(Vec<u8>),
    Hex(Vec<u8>),
    Array(Vec<Value>),
    Dict(Vec<(Vec<u8>, Value)>),
    Ref(Vec<u8>),
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

impl<'a> Parser<'a> {
    fn skip(&mut self) {
        while self.at < self.bytes.len() {
            match self.bytes[self.at] {
                byte if is_space(byte) => self.at += 1,
                b'%' => {
                    while self.at < self.bytes.len()
                        && !matches!(self.bytes[self.at], b'\n' | b'\r')
                    {
                        self.at += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn peek_keyword(&mut self, keyword: &[u8]) -> bool {
        self.skip();
        self.bytes[self.at..].starts_with(keyword)
            && self
                .bytes
                .get(self.at + keyword.len())
                .is_none_or(|&byte| is_space(byte) || is_delimiter(byte))
    }

    fn word(&mut self) -> Vec<u8> {
        let start = self.at;
        while self.at < self.bytes.len()
            && !is_space(self.bytes[self.at])
            && !is_delimiter(self.bytes[self.at])
        {
            self.at += 1;
        }
        self.bytes[start..self.at].to_vec()
    }

    /// One value; `refs` recognizes `n g R` references (never in content).
    fn value(&mut self, refs: bool) -> Value {
        self.skip();
        match self.bytes[self.at] {
            b'/' => {
                self.at += 1;
                let mut name = vec![b'/'];
                name.extend(self.word());
                Value::Name(name)
            }
            b'(' => {
                let start = self.at;
                let mut depth = 0;
                loop {
                    match self.bytes[self.at] {
                        b'\\' => self.at += 1,
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                self.at += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    self.at += 1;
                }
                Value::Literal(self.bytes[start..self.at].to_vec())
            }
            b'<' if self.bytes.get(self.at + 1) == Some(&b'<') => {
                self.at += 2;
                let mut entries = Vec::new();
                loop {
                    self.skip();
                    if self.bytes[self.at..].starts_with(b">>") {
                        self.at += 2;
                        break;
                    }
                    let Value::Name(key) = self.value(refs) else {
                        panic!("a PDF dictionary key is a name");
                    };
                    let value = self.value(refs);
                    entries.push((key, value));
                }
                Value::Dict(entries)
            }
            b'<' => {
                let start = self.at;
                while self.bytes[self.at] != b'>' {
                    self.at += 1;
                }
                self.at += 1;
                Value::Hex(self.bytes[start..self.at].to_vec())
            }
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                loop {
                    self.skip();
                    if self.bytes[self.at] == b']' {
                        self.at += 1;
                        break;
                    }
                    items.push(self.value(refs));
                }
                Value::Array(items)
            }
            _ => {
                let first = self.word();
                if refs && first.iter().all(u8::is_ascii_digit) {
                    let saved = self.at;
                    self.skip();
                    let second = self.word();
                    if !second.is_empty()
                        && second.iter().all(u8::is_ascii_digit)
                        && self.peek_keyword(b"R")
                    {
                        self.at += 1;
                        let mut reference = first;
                        reference.push(b' ');
                        reference.extend(second);
                        reference.extend(b" R");
                        return Value::Ref(reference);
                    }
                    self.at = saved;
                }
                Value::Atom(first)
            }
        }
    }
}

/// The bytes a literal string denotes.
fn literal_bytes(raw: &[u8]) -> Vec<u8> {
    let inner = &raw[1..raw.len() - 1];
    let mut out = Vec::new();
    let mut at = 0;
    while at < inner.len() {
        if inner[at] != b'\\' {
            out.push(inner[at]);
            at += 1;
            continue;
        }
        at += 1;
        match inner[at] {
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b'0'..=b'7' => {
                let mut code = 0_u32;
                let mut digits = 0;
                while digits < 3 && at < inner.len() && (b'0'..=b'7').contains(&inner[at]) {
                    code = code * 8 + u32::from(inner[at] - b'0');
                    at += 1;
                    digits += 1;
                }
                out.push(code as u8);
                continue;
            }
            b'\r' | b'\n' => {}
            other => out.push(other),
        }
        at += 1;
    }
    out
}

/// A text string as the `<FEFF...>` UTF-16BE hex the old writer spelled.
fn text_hex(value: &Value) -> Option<Vec<u8>> {
    let bytes = match value {
        Value::Literal(raw) => literal_bytes(raw),
        Value::Hex(raw) => {
            let digits: Vec<u8> = raw[1..raw.len() - 1]
                .iter()
                .copied()
                .filter(u8::is_ascii_hexdigit)
                .collect();
            digits
                .chunks(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        _ => return None,
    };
    let units: Vec<u16> = if bytes.starts_with(&[0xfe, 0xff]) {
        bytes[2..]
            .chunks(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect()
    } else {
        bytes.iter().map(|&byte| u16::from(byte)).collect()
    };
    let mut hex = b"<FEFF".to_vec();
    for unit in units {
        hex.extend(format!("{unit:04X}").into_bytes());
    }
    hex.push(b'>');
    Some(hex)
}

fn print(value: &Value, top: bool, out: &mut Vec<u8>) {
    match value {
        Value::Name(raw)
        | Value::Atom(raw)
        | Value::Literal(raw)
        | Value::Hex(raw)
        | Value::Ref(raw) => out.extend(raw),
        Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    out.push(b' ');
                }
                print(item, false, out);
            }
            out.push(b']');
        }
        Value::Dict(entries) => {
            out.extend(if top { &b"<<\n"[..] } else { &b"<< "[..] });
            for (key, value) in entries {
                out.extend(key);
                out.push(b' ');
                match (key.as_slice(), text_hex(value)) {
                    (b"/ActualText", Some(hex)) => out.extend(hex),
                    _ => print(value, false, out),
                }
                out.push(if top { b'\n' } else { b' ' });
            }
            out.extend(b">>");
        }
    }
}

fn get<'v>(entries: &'v [(Vec<u8>, Value)], key: &[u8]) -> Option<&'v Value> {
    entries
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

/// A content stream, one operator per line with its operands spaced.
fn normalize_content(data: &[u8]) -> Vec<u8> {
    let mut parser = Parser { bytes: data, at: 0 };
    let mut out = Vec::new();
    let mut operands: Vec<Value> = Vec::new();
    loop {
        parser.skip();
        if parser.at >= data.len() {
            break;
        }
        let value = parser.value(false);
        if let Value::Atom(raw) = &value {
            let operator = !raw.is_empty()
                && !raw[0].is_ascii_digit()
                && !matches!(raw[0], b'-' | b'+' | b'.')
                && !matches!(raw.as_slice(), b"true" | b"false" | b"null");
            if operator {
                for operand in operands.drain(..) {
                    print(&operand, false, &mut out);
                    out.push(b' ');
                }
                out.extend(raw);
                out.push(b'\n');
                continue;
            }
        }
        operands.push(value);
    }
    out
}

/// The PDF in the normalized layout described in the module documentation.
pub fn inflate_streams(pdf: &[u8]) -> Vec<u8> {
    let mut parser = Parser { bytes: pdf, at: 0 };
    let header_end = pdf
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(0, |end| end + 1);
    parser.at = header_end;
    let mut objects = Vec::new();
    let mut trailer = None;
    loop {
        parser.skip();
        if parser.at >= pdf.len() {
            break;
        }
        // A written file has a cross-reference table before its trailer; a
        // normalized one, read again, goes straight to the trailer. Objects
        // appended after a trailer, as an incremental update adds them, are
        // read as well.
        if parser.peek_keyword(b"xref") || parser.peek_keyword(b"trailer") {
            while !parser.peek_keyword(b"trailer") {
                parser.at += 1;
            }
            parser.at += 7;
            trailer = Some(parser.value(true));
            continue;
        }
        if parser.peek_keyword(b"startxref") {
            parser.at += 9;
            parser.skip();
            parser.word();
            continue;
        }
        let number = parser.word();
        parser.skip();
        let generation = parser.word();
        assert!(parser.peek_keyword(b"obj"), "a PDF object header");
        parser.at += 3;
        let value = parser.value(true);
        let mut stream = None;
        if parser.peek_keyword(b"stream") {
            parser.at += 6;
            if pdf[parser.at] == b'\r' {
                parser.at += 1;
            }
            parser.at += 1;
            let Value::Dict(entries) = &value else {
                panic!("a stream has a dictionary");
            };
            let Some(Value::Atom(length)) = get(entries, b"/Length") else {
                panic!("a stream declares a direct /Length");
            };
            let length: usize = std::str::from_utf8(length).unwrap().parse().unwrap();
            stream = Some(pdf[parser.at..parser.at + length].to_vec());
            parser.at += length;
            assert!(
                parser.peek_keyword(b"endstream"),
                "the declared /Length ends at endstream"
            );
            parser.at += 9;
        }
        assert!(parser.peek_keyword(b"endobj"), "a PDF object ends");
        parser.at += 6;
        objects.push((number, generation, value, stream));
    }
    let mut contents = BTreeSet::new();
    for (_, _, value, _) in &objects {
        if let Value::Dict(entries) = value {
            if matches!(get(entries, b"/Type"), Some(Value::Name(name)) if name == b"/Page") {
                match get(entries, b"/Contents") {
                    Some(Value::Ref(reference)) => {
                        contents.insert(reference.clone());
                    }
                    Some(Value::Array(items)) => {
                        for item in items {
                            if let Value::Ref(reference) = item {
                                contents.insert(reference.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let mut out = pdf[..header_end].to_vec();
    for (number, generation, value, stream) in objects {
        out.extend(&number);
        out.push(b' ');
        out.extend(&generation);
        out.extend(b" obj\n");
        match (value, stream) {
            (Value::Dict(mut entries), Some(mut data)) => {
                let flate = matches!(
                    get(&entries, b"/Filter"),
                    Some(Value::Name(name)) if name == b"/FlateDecode"
                ) || matches!(
                    get(&entries, b"/Filter"),
                    Some(Value::Array(items)) if matches!(items.as_slice(), [Value::Name(name)] if name == b"/FlateDecode")
                );
                if flate {
                    data = miniz_oxide::inflate::decompress_to_vec_zlib(&data)
                        .expect("a FlateDecode stream inflates as zlib");
                    entries.retain(|(key, _)| key != b"/Filter");
                }
                let mut reference = number.clone();
                reference.push(b' ');
                reference.extend(&generation);
                reference.extend(b" R");
                let content = contents.contains(&reference)
                    || matches!(get(&entries, b"/Subtype"), Some(Value::Name(name)) if name == b"/Form")
                    || matches!(get(&entries, b"/PatternType"), Some(Value::Atom(kind)) if kind == b"1");
                if content {
                    data = normalize_content(&data);
                }
                for (key, value) in &mut entries {
                    if key == b"/Length" {
                        *value = Value::Atom(data.len().to_string().into_bytes());
                    }
                }
                print(&Value::Dict(entries), true, &mut out);
                out.extend(b"\nstream\n");
                out.extend(&data);
                out.extend(b"\nendstream\nendobj\n");
            }
            (value, _) => {
                print(&value, true, &mut out);
                out.extend(b"\nendobj\n");
            }
        }
    }
    if let Some(trailer) = trailer {
        out.extend(b"trailer\n");
        print(&trailer, true, &mut out);
        out.push(b'\n');
    }
    out.extend(b"%%EOF\n");
    out
}

/// The decoded bytes of the attachment named `name` (`scene.excalidraw` or
/// `artifact-manifest.json`, spec 12.2), or `None` when the PDF has none.
pub fn pdf_attachment(pdf: &[u8], name: &str) -> Option<Vec<u8>> {
    let normalized = inflate_streams(pdf);
    let text = String::from_utf8_lossy(&normalized);
    let marker = format!("/F ({name})\n");
    let filespec = text
        .split(" 0 obj\n")
        .find(|object| object.contains("/Type /Filespec\n") && object.contains(&marker))?;
    let stream = filespec
        .split("/EF << /F ")
        .nth(1)?
        .split(' ')
        .next()?
        .to_owned();
    let header = format!("\n{stream} 0 obj\n");
    let start = find_bytes(&normalized, header.as_bytes())? + header.len();
    let body = start + find_bytes(&normalized[start..], b"\nstream\n")? + b"\nstream\n".len();
    let end = body + find_bytes(&normalized[body..], b"\nendstream\nendobj\n")?;
    Some(normalized[body..end].to_vec())
}

/// The normalized text of object `number`, from its header to `endobj`.
pub fn pdf_object(pdf: &str, number: usize) -> &str {
    let header = format!("\n{number} 0 obj\n");
    let start = pdf
        .find(&header)
        .map(|at| at + header.len())
        .unwrap_or_else(|| {
            panic!("the PDF has object {number}");
        });
    &pdf[start..start + pdf[start..].find("\nendobj\n").expect("the object ends")]
}

/// The numbers of the image XObjects that carry color: every image that is
/// not another image's soft mask.
pub fn color_images(pdf: &str) -> Vec<usize> {
    let images: Vec<(usize, &str)> = pdf
        .split('\n')
        .filter_map(|line| line.strip_suffix(" 0 obj"))
        .filter_map(|number| number.parse::<usize>().ok())
        .map(|number| (number, pdf_object(pdf, number)))
        .filter(|(_, object)| object.contains("/Subtype /Image\n"))
        .collect();
    let masks: Vec<usize> = images
        .iter()
        .filter_map(|(_, object)| reference(object, "/SMask "))
        .collect();
    images
        .into_iter()
        .map(|(number, _)| number)
        .filter(|number| !masks.contains(number))
        .collect()
}

/// The object number `key` refers to in `dictionary`, if it names a reference.
pub fn reference(dictionary: &str, key: &str) -> Option<usize> {
    let rest = dictionary.split(key).nth(1)?;
    let mut tokens = rest.split([' ', '\n', ']', '>']);
    let number = tokens.next()?.parse().ok()?;
    (tokens.next()? == "0" && tokens.next()? == "R").then_some(number)
}

/// The resolved color space of image `number`: Krilla binds canonical RGB
/// through an indirect `[/ICCBased ...]` array (ADR-0021).
pub fn image_color_space(pdf: &str, number: usize) -> String {
    let image = pdf_object(pdf, number);
    match reference(image, "/ColorSpace ") {
        Some(space) => pdf_object(pdf, space).trim().to_owned(),
        None => image
            .split("/ColorSpace ")
            .nth(1)
            .and_then(|rest| rest.split('\n').next())
            .expect("the image declares a color space")
            .to_owned(),
    }
}

/// Whether every color image is in an ICC-based RGB space with a gray soft
/// mask bound, as the renderer embeds rasters.
pub fn images_are_icc_rgb_with_soft_masks(pdf: &str) -> bool {
    let images = color_images(pdf);
    !images.is_empty()
        && images.iter().all(|&number| {
            let image = pdf_object(pdf, number);
            image_color_space(pdf, number).starts_with("[/ICCBased ")
                && reference(image, "/SMask ")
                    .is_some_and(|mask| pdf_object(pdf, mask).contains("/ColorSpace /DeviceGray\n"))
        })
}

/// The decoded bytes of object `number`'s stream, byte for byte.
pub fn object_stream(pdf: &[u8], number: usize) -> Vec<u8> {
    let normalized = inflate_streams(pdf);
    let header = format!("\n{number} 0 obj\n");
    let start =
        find_bytes(&normalized, header.as_bytes()).expect("the object exists") + header.len();
    let body = start
        + find_bytes(&normalized[start..], b"\nstream\n").expect("the object has a stream")
        + b"\nstream\n".len();
    let end =
        body + find_bytes(&normalized[body..], b"\nendstream\nendobj\n").expect("the stream ends");
    normalized[body..end].to_vec()
}

/// The page's normalized content stream: one operator per line.
pub fn page_content(pdf: &[u8]) -> String {
    let text = pdf_text(pdf);
    let page = text
        .split(" 0 obj\n")
        .find(|object| object.contains("/Type /Page\n"))
        .expect("the PDF has a page");
    let contents = reference(page, "/Contents ").expect("the page names its content stream");
    let object = pdf_object(&text, contents);
    object
        .split_once("\nstream\n")
        .expect("the content is a stream")
        .1
        .trim_end_matches("\nendstream")
        .to_owned()
}

/// The 2-byte character codes the text-showing operators of `content` show,
/// in order: every code in the literal and hexadecimal strings of its `Tj`
/// and `TJ` operators, whichever Krilla chose for the run (ADR-0021).
pub fn shown_codes(content: &str) -> Vec<u16> {
    let mut codes = Vec::new();
    for line in content.lines() {
        let Some(operands) = line
            .strip_suffix(" Tj")
            .or_else(|| line.strip_suffix(" TJ"))
        else {
            continue;
        };
        let bytes = operands.as_bytes();
        let mut at = 0;
        let mut shown = Vec::new();
        while at < bytes.len() {
            match bytes[at] {
                b'(' => {
                    let start = at;
                    let mut depth = 0;
                    loop {
                        match bytes[at] {
                            b'\\' => at += 1,
                            b'(' => depth += 1,
                            b')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        at += 1;
                    }
                    shown.extend(literal_bytes(&bytes[start..=at]));
                }
                b'<' => {
                    let end = at + operands[at..].find('>').expect("hex string ends");
                    let digits: Vec<u8> = bytes[at + 1..end]
                        .iter()
                        .copied()
                        .filter(u8::is_ascii_hexdigit)
                        .collect();
                    shown.extend(digits.chunks(2).map(|pair| {
                        u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()
                    }));
                    at = end;
                }
                _ => {}
            }
            at += 1;
        }
        codes.extend(
            shown
                .chunks(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]])),
        );
    }
    codes
}

/// Each embedded font program's source, from its Modified Version notice in
/// the manifest (ADR-0021): the manifested identity it was derived from and
/// the SHA-256 of that source program.
pub fn pdf_font_program_sources(pdf: &[u8]) -> Vec<(String, String)> {
    let manifest: serde_json::Value =
        serde_json::from_str(&pdf_manifest(pdf).expect("a font-bearing PDF carries its manifest"))
            .expect("the manifest is JSON");
    manifest["notices"]
        .as_array()
        .expect("the manifest lists notices")
        .iter()
        .filter(|notice| {
            notice["asset_identity"]
                .as_str()
                .is_some_and(|identity| identity.starts_with("pdf-font-program@sha256:"))
        })
        .map(|notice| {
            let text = notice["text"].as_str().expect("a notice has text");
            let identity = text
                .strip_prefix("Derived font program of ")
                .and_then(|rest| rest.split(". Source program SHA-256: ").next())
                .expect("the notice names its source");
            let digest = text
                .split(". Source program SHA-256: ")
                .nth(1)
                .and_then(|rest| rest.split('.').next())
                .expect("the notice names its source digest");
            (identity.to_owned(), digest.to_owned())
        })
        .collect()
}

/// The artifact manifest's canonical JSON, which PDF carries as the
/// `artifact-manifest.json` attachment (spec 12.2, ADR-0021).
pub fn pdf_manifest(pdf: &[u8]) -> Option<String> {
    pdf_attachment(pdf, "artifact-manifest.json")
        .map(|bytes| String::from_utf8(bytes).expect("the manifest is UTF-8 JSON"))
}

/// The artifact as text with every Flate stream inflated: what a test reads
/// when it looks for operators, dictionary entries or CMap lines.
pub fn pdf_text(pdf: &[u8]) -> String {
    String::from_utf8_lossy(&inflate_streams(pdf)).into_owned()
}

/// The body of the ToUnicode CMap of the family `cmap_name` names, spelled
/// `/CMapName /<family>-UCS def` as the tests have always named it. Krilla
/// calls every CMap `Custom` (ADR-0021), so the CMap is found through the
/// family's Type0 font.
pub fn to_unicode_cmap<'a>(pdf: &'a str, cmap_name: &str) -> &'a str {
    let family = cmap_name
        .strip_prefix("/CMapName /")
        .and_then(|rest| rest.strip_suffix("-UCS def"))
        .unwrap_or(cmap_name);
    let header = format!(
        "<<\n/Type /Font\n/Subtype /Type0\n/BaseFont /{}\n",
        base_font(pdf, family)
    );
    let font = pdf
        .split(" 0 obj\n")
        .find(|object| object.starts_with(&header))
        .unwrap_or_else(|| panic!("the PDF carries the ToUnicode CMap of {family}"));
    let cmap = reference(font, "/ToUnicode ").expect("the font names its ToUnicode CMap");
    let object = pdf_object(pdf, cmap);
    let body = object
        .split_once("\nstream\n")
        .expect("the CMap is a stream")
        .1;
    body.split("endcmap").next().unwrap_or(body)
}

/// The English family name (name ID 1) of an embedded face.
pub fn family_name(face: &ttf_parser::Face<'_>) -> String {
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
            .as_chunks::<4>()
            .0
            .iter()
            .map(|digits| {
                u16::from_str_radix(std::str::from_utf8(digits).expect("hex is ASCII"), 16)
                    .expect("ActualText is hexadecimal")
            })
            .collect::<Vec<_>>();
        extracted.push_str(&String::from_utf16(&units).expect("ActualText is UTF-16"));
    }
    extracted
}

/// Every embedded `FontFile2` program, decoded, in the order the font
/// descriptors name them.
pub fn embedded_pdf_fonts(pdf: &[u8]) -> Vec<Vec<u8>> {
    let text = pdf_text(pdf);
    text.split("/FontFile2 ")
        .skip(1)
        .filter_map(|rest| {
            let number = rest.split(' ').next()?.parse::<usize>().ok()?;
            Some(object_stream(pdf, number))
        })
        .collect()
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
/// subset tag included.
pub fn type0_base_fonts(pdf: &str) -> Vec<&str> {
    pdf.split("/Type /Font\n/Subtype /Type0\n/BaseFont /")
        .skip(1)
        .map(|tail| tail.split('\n').next().expect("the base font line ends"))
        .collect()
}

/// The program name once its subset tag — six uppercase letters and `+`,
/// which Krilla gives every embedded program (ADR-0021) — is removed.
pub fn strip_subset_tag(base_font: &str) -> &str {
    match base_font.split_once('+') {
        Some((tag, name))
            if tag.len() == 6 && tag.bytes().all(|byte| byte.is_ascii_uppercase()) =>
        {
            name
        }
        _ => base_font,
    }
}

/// The PostScript family prefix of the program PDF embeds for `pdf_name`:
/// the effective alias, which replaces the Reserved Font Names of Virgil,
/// Cascadia Code, Lilita One and Liberation Sans (ADR-0001), with spaces as
/// hyphens.
pub fn program_family(pdf_name: &str) -> &str {
    match pdf_name {
        "Virgil" => "ExcalidrawRender-1",
        "CascadiaCode" => "ExcalidrawRender-3",
        "LilitaOne" => "ExcalidrawRender-7",
        "LiberationSans" => "ExcalidrawRender-9",
        "ComicShanns" => "Comic-Shanns",
        "NotoEmoji" => "Noto-Emoji",
        other => other,
    }
}

/// Whether `base_font` is a program of the family `pdf_name`: its renamed
/// PostScript name is `<family>-Regular-<identity prefix>` (ADR-0021).
pub fn is_family_program(base_font: &str, pdf_name: &str) -> bool {
    strip_subset_tag(base_font)
        .strip_prefix(program_family(pdf_name))
        .and_then(|rest| rest.strip_prefix("-Regular-"))
        .is_some_and(|prefix| prefix.len() == 32 && prefix.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Whether the PDF declares a Type0 font for `pdf_name`.
pub fn declares_base_font(pdf: &str, pdf_name: &str) -> bool {
    type0_base_fonts(pdf)
        .into_iter()
        .any(|base_font| is_family_program(base_font, pdf_name))
}

/// The declared `/BaseFont` of `pdf_name`'s first Type0 font, tag included.
pub fn base_font<'a>(pdf: &'a str, pdf_name: &str) -> &'a str {
    type0_base_fonts(pdf)
        .into_iter()
        .find(|base_font| is_family_program(base_font, pdf_name))
        .unwrap_or_else(|| panic!("the PDF declares a Type0 font for {pdf_name}"))
}

/// The two file identifiers of the trailer `/ID`, as the PDF spells them:
/// Krilla's literal strings, parentheses included (ADR-0021).
pub fn trailer_file_identifiers(pdf: &[u8]) -> (String, String) {
    let text = String::from_utf8_lossy(&inflate_streams(pdf)).into_owned();
    let trailer = text
        .rsplit("\ntrailer\n")
        .next()
        .expect("the PDF ends in a trailer");
    let ids = trailer
        .split("/ID [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("the trailer carries /ID");
    let (first, second) = ids.split_once(' ').expect("/ID carries two strings");
    (first.to_owned(), second.to_owned())
}

/// The offset of the first `needle` in `haystack`.
pub fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
