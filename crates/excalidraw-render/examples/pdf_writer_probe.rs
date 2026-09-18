//! Bounded #186 API experiment, not a production backend or allocation adapter.
//! Run through tools/pdf-harness/probe-pdf-writer.py for independent validation.

use std::{error::Error, path::PathBuf};

use pdf_writer::{Content, Filter, Finish, Name, Pdf, Primitive, Ref};

/// Only validated fixture tokens can cross the raw Primitive interface. A
/// production adapter would take tokens from the existing format_pdf_number.
#[derive(Clone, Copy)]
struct ExactNumber<'a>(&'a str);

impl<'a> ExactNumber<'a> {
    fn new(token: &'a str, expected: f64) -> Result<Self, &'static str> {
        let unsigned = token.strip_prefix('-').unwrap_or(token);
        let mut parts = unsigned.split('.');
        let integer = parts.next().unwrap_or_default();
        let fraction = parts.next();
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !expected.is_finite()
            || !digits(integer)
            || fraction.is_some_and(|s| !digits(s))
            || parts.next().is_some()
            || (expected == 0.0 && token != "0")
            || token.parse::<f64>().ok() != Some(expected)
        {
            return Err("not one finite, round-tripping PDF decimal token");
        }
        Ok(Self(token))
    }
}

impl Primitive for ExactNumber<'_> {
    fn write(self, buf: &mut Vec<u8>) {
        // This infallible Vec interface is precisely the upstream allocation gap.
        buf.extend_from_slice(self.0.as_bytes());
    }
}

const XMP: &[u8] = br#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:title><rdf:Alt><rdf:li xml:lang="x-default">pdf-writer #186 probe</rdf:li></rdf:Alt></dc:title>
</rdf:Description></rdf:RDF>
<!-- trailer
This deliberately exercises the post-finish adapter's suffix boundary. -->
</x:xmpmeta>
<?xpacket end="w"?>"#;

fn content() -> Vec<u8> {
    let mut content = Content::new();
    content.op("q");
    content.op("rg").operands([1, 1, 1]);
    content.op("re").operands([0, 0, 144, 108]);
    content.op("f");
    content.op("cm").operands([
        ExactNumber::new("1", 1.0).unwrap(),
        ExactNumber::new("0", 0.0).unwrap(),
        ExactNumber::new("0", -0.0).unwrap(),
        ExactNumber::new("1", 1.0).unwrap(),
        ExactNumber::new("0.30000000000000004", 0.1 + 0.2).unwrap(),
        ExactNumber::new("-0.0000000000000001", -1e-16).unwrap(),
    ]);
    content.op("rg").operands([
        ExactNumber::new("0.2", 0.2).unwrap(),
        ExactNumber::new("0.4", 0.4).unwrap(),
        ExactNumber::new("0.8", 0.8).unwrap(),
    ]);
    content.op("re").operands([
        ExactNumber::new("12.123456789012345", 12.123456789012345).unwrap(),
        ExactNumber::new("18.000000000000004", 18.000000000000004).unwrap(),
        ExactNumber::new("96.00000000000001", 96.00000000000001).unwrap(),
        ExactNumber::new("54.123456789012344", 54.123456789012344).unwrap(),
    ]);
    content.op("f");
    content.op("RG").operands([0, 0, 0]);
    content
        .op("w")
        .operand(ExactNumber::new("1.125", 1.125).unwrap());
    content.op("m").operands([12, 84]);
    content.op("c").operands([36, 12, 72, 108, 132, 24]);
    content.op("S");
    content.op("Q");
    content.finish()
}

fn candidate() -> Vec<u8> {
    // The fixed five-object graph is bounded and closed. This experiment is not
    // a dynamic checked-ID registry, nor a claim that pdf-writer validates refs.
    let [catalog, pages, page, stream, metadata] = [1, 2, 3, 4, 5].map(Ref::new);
    let mut pdf = Pdf::new();
    pdf.catalog(catalog).pages(pages).metadata(metadata);
    pdf.pages(pages).kids([page]).count(1);
    let mut page_writer = pdf.page(page);
    page_writer.parent(pages).contents(stream);
    page_writer.insert(Name(b"MediaBox")).array().items([
        ExactNumber::new("0", 0.0).unwrap(),
        ExactNumber::new("0", 0.0).unwrap(),
        ExactNumber::new("144.00000000000003", 144.00000000000003).unwrap(),
        ExactNumber::new("108.00000000000001", 108.00000000000001).unwrap(),
    ]);
    page_writer.resources().finish();
    page_writer.finish();
    // Same level/configuration as ADR-0008, but this convenience function and
    // all writer allocations are infallible. Production keeps deflate_stream.
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&content(), 6);
    pdf.stream(stream, &compressed).filter(Filter::FlateDecode);
    pdf.metadata(metadata, XMP);
    assert_eq!(
        pdf.refs().map(Ref::get).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    // No claim that set_file_id can implement ADR-0008: the independent probe
    // explicitly retains a handwritten post-finish trailer adapter.
    pdf.finish()
}

fn check_numeric_boundary() {
    for value in [
        -0.0,
        0.1 + 0.2,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        -1e-16,
        16_777_217.0,
        1e21,
        f64::MAX,
    ] {
        let token = if value == 0.0 {
            "0".into()
        } else {
            value.to_string()
        };
        let number = ExactNumber::new(&token, value).expect("exact decimal fixture");
        let mut content = Content::new();
        content.op("w").operand(number);
        assert_eq!(content.finish(), format!("{token} w").as_bytes());
    }
    for token in ["", "NaN", "inf", "1e0", "+1", "1 0 R", "1\nQ", "1.0.0"] {
        assert!(ExactNumber::new(token, 1.0).is_err(), "{token:?}");
    }
    assert!(ExactNumber::new("-0", -0.0).is_err());
    assert!(ExactNumber::new("0", f64::NAN).is_err());
    assert!(ExactNumber::new("0", f64::INFINITY).is_err());
    assert!(ExactNumber::new("0.3", 0.1 + 0.2).is_err());
    assert_ne!(f64::from((0.1_f64 + 0.2) as f32), 0.1 + 0.2);
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(
        args.next()
            .ok_or("usage: pdf_writer_probe <output-directory>")?,
    );
    if args.next().is_some() || !output.is_dir() {
        return Err("supply one existing output directory".into());
    }
    check_numeric_boundary();
    let bytes = candidate();
    assert_eq!(bytes, candidate(), "repeat serialization");
    std::fs::write(output.join("candidate-raw.pdf"), bytes)?;
    std::fs::write(output.join("content.bin"), content())?;
    std::fs::write(output.join("metadata.xmp"), XMP)?;
    Ok(())
}
