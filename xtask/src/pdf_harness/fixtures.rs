//! Hand-authored qualification PDFs, independent of the production PDF writer.
//! Keep serialization byte-identical to tools/pdf-harness/generate-fixtures.py.

use crate::conformance::{repo_root, sha256};
use std::fs;
use std::path::Path;

const SVG_PATH: [&str; 8] = [
    "0 0 m 60.36 0.51 117.93 -1.29 180 0 c",
    "0 0 m 46.23 0.2 91.7 -0.37 180 0 c",
    "180 0 m 180.82 21.75 178.03 45.56 180 100 c",
    "180 0 m 178.91 21.23 180.01 41.46 180 100 c",
    "180 100 m 116.02 98.39 52.73 98.26 0 100 c",
    "180 100 m 124.78 101.1 71.18 100.93 0 100 c",
    "0 100 m 0.8 76.02 2.55 50.08 0 0 c",
    "0 100 m -0.56 64.79 -0.37 31.41 0 0 c",
];

fn fixed(value: f64) -> [u8; 4] {
    ((value * 65536.0).round_ties_even() as i32).to_be_bytes()
}

fn xyz(x: f64, y: f64, z: f64) -> Vec<u8> {
    let mut data = b"XYZ \0\0\0\0".to_vec();
    data.extend(fixed(x));
    data.extend(fixed(y));
    data.extend(fixed(z));
    data
}

fn curve(gamma: f64) -> Vec<u8> {
    let mut data = b"curv\0\0\0\0".to_vec();
    data.extend(1_u32.to_be_bytes());
    data.extend(((gamma * 256.0).round_ties_even() as u16).to_be_bytes());
    data.extend([0, 0]);
    data
}

fn icc_profile() -> Vec<u8> {
    let tags = [
        (b"rXYZ", xyz(0.4360747, 0.2225045, 0.0139322)),
        (b"gXYZ", xyz(0.3850649, 0.7168786, 0.0971045)),
        (b"bXYZ", xyz(0.1430804, 0.0606169, 0.7141733)),
        (b"wtpt", xyz(0.9642, 1.0, 0.8249)),
        (b"rTRC", curve(2.2)),
        (b"gTRC", curve(2.2)),
        (b"bTRC", curve(2.2)),
    ];
    let data_offset = 128 + 4 + tags.len() * 12;
    let mut table = (tags.len() as u32).to_be_bytes().to_vec();
    let mut payload = Vec::new();
    for (signature, data) in tags {
        while !(data_offset + payload.len()).is_multiple_of(4) {
            payload.push(0);
        }
        table.extend(signature);
        table.extend(((data_offset + payload.len()) as u32).to_be_bytes());
        table.extend((data.len() as u32).to_be_bytes());
        payload.extend(data);
    }

    let size = 128 + table.len() + payload.len();
    let mut header = vec![0; 128];
    header[0..4].copy_from_slice(&(size as u32).to_be_bytes());
    header[4..8].copy_from_slice(b"ERND");
    header[8..12].copy_from_slice(b"\x02\x10\0\0");
    header[12..16].copy_from_slice(b"mntr");
    header[16..20].copy_from_slice(b"RGB ");
    header[20..24].copy_from_slice(b"XYZ ");
    for (index, value) in [2000_u16, 1, 1, 0, 0, 0].into_iter().enumerate() {
        header[24 + index * 2..26 + index * 2].copy_from_slice(&value.to_be_bytes());
    }
    header[36..40].copy_from_slice(b"acsp");
    header[48..52].copy_from_slice(b"ERND");
    header[52..56].copy_from_slice(b"sRGB");
    header[68..72].copy_from_slice(&fixed(0.9642));
    header[72..76].copy_from_slice(&fixed(1.0));
    header[76..80].copy_from_slice(&fixed(0.8249));
    header[80..84].copy_from_slice(b"ERND");
    header.extend(table);
    header.extend(payload);
    header
}

fn stream(dictionary: &[u8], data: &[u8]) -> Vec<u8> {
    let mut output = b"<< ".to_vec();
    output.extend(dictionary);
    output.extend(format!(" /Length {} >>\nstream\n", data.len()).as_bytes());
    output.extend(data);
    output.extend(b"\nendstream");
    output
}

fn reference_raster() -> Result<Vec<u8>, String> {
    let path = repo_root().join(
        "tests/conformance/svg-viewer-qualification/rectangle-outline/evidence/chromium-reference.rgba",
    );
    let data = fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if data.len() != 200 * 120 * 4 {
        return Err("canonical Chromium rectangle raster has invalid length".into());
    }
    if data.chunks_exact(4).any(|pixel| pixel[3] != 255) {
        return Err("calibration control requires an opaque reference raster".into());
    }
    Ok(data)
}

fn raster_runs(data: &[u8]) -> Vec<String> {
    let mut operations = vec!["1 1 1 rg".into(), "0 0 200 120 re".into(), "f".into()];
    for y in 0..120 {
        let mut x = 0;
        while x < 200 {
            let offset = (y * 200 + x) * 4;
            let color = &data[offset..offset + 3];
            let mut end = x + 1;
            while end < 200 {
                let other = (y * 200 + end) * 4;
                if &data[other..other + 3] != color {
                    break;
                }
                end += 1;
            }
            if color != b"\xff\xff\xff" {
                let red = f64::from(color[0]) / 255.0;
                let green = f64::from(color[1]) / 255.0;
                let blue = f64::from(color[2]) / 255.0;
                operations.extend([
                    format!("{red:.12} {green:.12} {blue:.12} rg"),
                    format!("{x} {y} {} 1 re", end - x),
                    "f".into(),
                ]);
            }
            x = end;
        }
    }
    operations
}

fn content(visual_failure: bool) -> Result<Vec<u8>, String> {
    let mut operations: Vec<String> = [
        "q",
        "0.75 0 0 -0.75 0 90 cm",
        "1 1 1 rg",
        "0 0 200 120 re",
        "f",
        "1 0 0 1 10 10 cm",
        "/GS1 gs",
        "2 w",
        "1 J",
        "0.117647059 0.117647059 0.117647059 RG",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    operations.extend(SVG_PATH.map(String::from));
    operations.extend(["S".into(), "1 0 0 1 -10 -10 cm".into()]);
    operations.extend(raster_runs(&reference_raster()?));
    if visual_failure {
        operations.extend(["0.8 0 0 rg".into(), "40 30 40 40 re".into(), "f".into()]);
    }
    operations.extend(["Q".into(), String::new()]);
    Ok(operations.join("\n").into_bytes())
}

fn build_pdf(visual_failure: bool, image: bool, active_action: bool) -> Result<Vec<u8>, String> {
    let profile = icc_profile();
    let mut resources = "/ExtGState << /GS1 5 0 R >>".to_owned();
    if image {
        resources.push_str(" /XObject << /UndeclaredForm 8 0 R >>");
    }
    let mut catalog = "<< /Type /Catalog /Pages 2 0 R /OutputIntents [7 0 R]".to_owned();
    if active_action {
        catalog.push_str(" /OpenAction 8 0 R");
    }
    catalog.push_str(" >>");
    let mut objects = vec![
        catalog.into_bytes(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 150 90] /CropBox [0 0 150 90] /Resources << {resources} >> /Contents 4 0 R >>").into_bytes(),
        stream(b"", &content(visual_failure)?),
        b"<< /Type /ExtGState /CA 1 /ca 1 /BM /Normal >>".to_vec(),
        stream(b"/N 3 /Alternate /DeviceRGB", &profile),
        b"<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB) /Info (excalidraw-render project sRGB) /DestOutputProfile 6 0 R >>".to_vec(),
    ];
    if image {
        objects.push(stream(
            b"/Type /XObject /Subtype /Form /BBox [0 0 1 1] /Resources << /XObject << /UndeclaredImage 9 0 R >> >>",
            b"",
        ));
        objects.push(stream(
            b"/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8",
            b"\xff\0\0",
        ));
    } else if active_action {
        objects.push(b"<< /S /JavaScript /JS (app.alert\\(1\\)) >>".to_vec());
    }
    Ok(serialize_pdf(&objects))
}

fn build_structure_coverage_pdf() -> Vec<u8> {
    let profile = icc_profile();
    let xmp = concat!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">",
        "<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">",
        "<rdf:Description xmlns:er=\"https://excalidraw-render.invalid/manifest/1\" ",
        "er:artifact_manifest=\"{&quot;schema_version&quot;:1}\" er:notices=\"coverage\"/>",
        "</rdf:RDF></x:xmpmeta>",
    );
    let to_unicode = b"/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (ERND) /Ordering (Coverage) /Supplement 0 >> def
/CMapName /Coverage def
/CMapType 2 def
1 begincodespacerange
<0000> <ffff>
endcodespacerange
1 beginbfchar
<0001> <0041>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end
";
    let page_content = b"q\n0 0 150 90 re\nW\nn\n/MaskGS gs\n/CoverageForm Do\nQ\n";
    let form_content = b"/Span << /ActualText (A) >> BDC\nBT\n/F1 12 Tf\n0 0 Td\n<0001> Tj\nET\nEMC\n/SourceImage Do\n";
    let mask_content = b"0 g\n0 0 10 10 re\nf\n";
    let attachment = br#"{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}"#;
    let fake_sfnt = b"\0\x01\0\0\0\x01\0\x10\0\0\0\0ERND\0\0\0\0\0\0\0\x0c\0\0\0\x04test";
    let objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R /OutputIntents [6 0 R] /Metadata 11 0 R /Names << /EmbeddedFiles 15 0 R >> >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 150 90] /CropBox [0 0 150 90] /Resources << /ExtGState << /MaskGS 10 0 R >> /XObject << /CoverageForm 7 0 R >> >> /Contents 4 0 R /Annots [14 0 R] >>".to_vec(),
        stream(b"", page_content),
        stream(b"/N 3 /Alternate /DeviceRGB", &profile),
        b"<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB) /Info (excalidraw-render project sRGB) /DestOutputProfile 5 0 R >>".to_vec(),
        stream(
            b"/Type /XObject /Subtype /Form /BBox [0 0 10 10] /Group << /S /Transparency /I true /K false /CS /DeviceRGB >> /Resources << /Font << /F1 8 0 R >> /XObject << /SourceImage 13 0 R >> >>",
            form_content,
        ),
        b"<< /Type /Font /Subtype /Type0 /BaseFont /Coverage /Encoding /Identity-H /DescendantFonts [9 0 R] /ToUnicode 12 0 R >>".to_vec(),
        b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Coverage /CIDSystemInfo << /Registry (ERND) /Ordering (Coverage) /Supplement 0 >> /FontDescriptor 16 0 R /CIDToGIDMap /Identity >>".to_vec(),
        b"<< /Type /ExtGState /CA 0.75 /ca 0.5 /BM /Multiply /SMask << /S /Alpha /G 17 0 R >> >>".to_vec(),
        stream(b"/Type /Metadata /Subtype /XML", xmp.as_bytes()),
        stream(b"", to_unicode),
        stream(
            b"/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /SMask 21 0 R",
            b"\0\x80\xff",
        ),
        b"<< /Type /Annot /Subtype /Link /Rect [0 0 10 10] /Border [0 0 0] /A << /S /URI /URI (https://example.com/) >> >>".to_vec(),
        b"<< /Names [(scene.excalidraw) 18 0 R] >>".to_vec(),
        b"<< /Type /FontDescriptor /FontName /Coverage /Flags 4 /FontBBox [0 -200 1000 800] /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 20 0 R >>".to_vec(),
        stream(
            b"/Type /XObject /Subtype /Form /BBox [0 0 10 10] /Group << /S /Transparency /I true /K false /CS /DeviceGray >> /Resources << >>",
            mask_content,
        ),
        b"<< /Type /Filespec /F (scene.excalidraw) /UF (scene.excalidraw) /Desc (Excalidraw scene) /EF << /F 19 0 R /UF 19 0 R >> >>".to_vec(),
        stream(
            format!("/Type /EmbeddedFile /Subtype /application#2Fvnd.excalidraw+json /Params << /Size {} >>", attachment.len()).as_bytes(),
            attachment,
        ),
        stream(b"/Length1 32", fake_sfnt),
        stream(
            b"/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            b"\x80",
        ),
    ];
    serialize_pdf(&objects)
}

fn serialize_pdf(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut output = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(output.len());
        output.extend(format!("{} 0 obj\n", index + 1).as_bytes());
        output.extend(object);
        output.extend(b"\nendobj\n");
    }
    let xref = output.len();
    output.extend(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    output.extend(b"0000000000 65535 f \n");
    for offset in offsets {
        output.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    output.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    output
}

fn write(path: &Path, data: &[u8]) -> Result<(), String> {
    fs::write(path, data).map_err(|error| format!("write {}: {error}", path.display()))?;
    println!(
        "{} {} {}",
        path.file_name().unwrap().to_string_lossy(),
        data.len(),
        sha256(data)
    );
    Ok(())
}

pub(super) fn generate(output: &Path) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|error| format!("create {}: {error}", output.display()))?;
    let valid = build_pdf(false, false, false)?;
    write(&output.join("candidate-control.pdf"), &valid)?;
    write(
        &output.join("visual-fail-structure-pass.pdf"),
        &build_pdf(true, false, false)?,
    )?;
    write(
        &output.join("visual-pass-structure-fail.pdf"),
        &build_pdf(false, true, false)?,
    )?;
    write(
        &output.join("structure-coverage.pdf"),
        &build_structure_coverage_pdf(),
    )?;
    write(
        &output.join("active-action.pdf"),
        &build_pdf(false, false, true)?,
    )?;
    let marker = b"startxref\n";
    let start = valid
        .windows(marker.len())
        .rposition(|window| window == marker)
        .ok_or("generated PDF is missing startxref")?
        + marker.len();
    let end = start
        + valid[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or("generated PDF has an unterminated startxref")?;
    let mut warning = valid[..start].to_vec();
    warning.push(b'1');
    warning.extend(&valid[end..]);
    write(&output.join("parser-warning.pdf"), &warning)
}

#[cfg(test)]
mod tests {
    use super::{curve, fixed};

    #[test]
    fn icc_rounding_matches_python_ties_to_even() {
        for (scaled, expected) in [(0.5, 0_i32), (1.5, 2), (2.5, 2), (-0.5, 0), (-1.5, -2)] {
            assert_eq!(fixed(scaled / 65536.0), expected.to_be_bytes());
        }
        for (scaled, expected) in [(0.5, 0_u16), (1.5, 2), (2.5, 2)] {
            assert_eq!(&curve(scaled / 256.0)[12..14], &expected.to_be_bytes());
        }
    }
}
